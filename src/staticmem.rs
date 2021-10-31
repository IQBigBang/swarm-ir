use std::{collections::HashMap, io::{Cursor, Write}};

use crate::{abi::Abi, module::Module, ty::{Ty, Type}};

/// Static memory is the memory whose contents are known at compile-time
/// but must remain addressable at runtime.
/// 
/// The difference between static memory and globals is that globals
/// can only contain scalar values and do not have a runtime memory address,
/// whereas the items in the static memory have a well-defined address.
pub struct StaticMemory {
    items: Vec<SMItem>
}

impl StaticMemory {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self { items: vec![] }
    }

    /// Add an item to static memory.
    /// 
    /// Once the item is added, it CANNOT be modified in any way
    pub fn add_item(&mut self, item: SMItem) -> SMItemRef {
        self.items.push(item);
        SMItemRef(self.items.len() - 1)
    }

    pub fn lookup_item(&self, item_ref: SMItemRef) -> &'_ SMItem {
        &self.items[item_ref.0]
    }
}

/// A single item inside the static memory
pub struct SMItem {
    pub value: SMValue,
    /// The mutability of this item.
    /// If this is *const*, the value is assumed to never change at runtime.
    pub mutability: Mutability,
    /// True if this item's address must be unique.
    /// 
    /// If this is false, multiple items with the same value and mutability may be merged
    /// into one to save space.
    pub unique: bool,
}

#[derive(Clone)]
pub enum Mutability { Const, Mut }

#[derive(Clone)]
pub enum Sign { S, U }

/// A value inside the static memory
#[derive(Clone)]
pub enum SMValue {
    Int8(u8, Sign),
    Int16(u16, Sign),
    Int32(u32, Sign),
    Float(f32),
    Struct(Vec<SMValue>),
    /// Arbitrary bytes
    Blob(Box<[u8]>),
    /// A pointer to another part of the static memory
    PtrTo(SMItemRef),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct SMItemRef(usize);

pub(crate) struct CompiledStaticMemory {
    /// The resulting memory as a series of bytes
    pub(crate) buf: Vec<u8>,
    /// The addresses of items inside the result memory
    pub(crate) addresses: HashMap<SMItemRef, usize>,
}

impl CompiledStaticMemory {
    /// Compile the static memory.
    /// 
    /// Doesn't add the memory to the module in any way, the module
    /// is required because of types.
    pub fn compile<A: Abi>(m: &Module, mem: &StaticMemory) -> Self {
        let mut addresses = HashMap::new();
        // First, calculate the addresses
        // the addresses START at eight to avoid making a null pointer valid
        let mut curr_address = 8usize;
        for (i, item) in mem.items.iter().enumerate() {
            let ty = Self::get_item_type(&item.value, m);
            let size = A::type_sizeof(ty);
            let align = 2_usize.pow(A::type_alignment(ty) as u32);
            if curr_address % align != 0 {
                curr_address += align - (curr_address % align);
            }
            addresses.insert(SMItemRef(i), curr_address);
            curr_address += size;
        }
        // Then actually insert the data into memory
        // First, zero-initialize
        let buf = vec![0; curr_address];
        let mut cur = Cursor::new(buf);
        // Then write every item to the memory
        for (n, item) in mem.items.iter().enumerate() {
            // position the cursor to the address of the item
            cur.set_position(addresses[&SMItemRef(n)] as u64);
            Self::write_to_memory::<A>(&mut cur, &item.value, m, &addresses);
        }
        
        CompiledStaticMemory { buf: cur.into_inner(), addresses }
    }

    fn write_to_memory<A: Abi>(place: &mut Cursor<Vec<u8>>, item: &SMValue, m: &Module, addresses: &HashMap<SMItemRef, usize>) {
        match item {
            SMValue::Int8(val, _) => { place.write_all(&[*val]).unwrap(); },
            SMValue::Int16(val, _) => {
                if A::is_little_endian() {
                    place.write_all(&val.to_le_bytes()).unwrap();
                } else {
                    place.write_all(&val.to_be_bytes()).unwrap();
                }
            }
            SMValue::Int32(val, _) => {
                if A::is_little_endian() {
                    place.write_all(&val.to_le_bytes()).unwrap();
                } else {
                    place.write_all(&val.to_be_bytes()).unwrap();
                }
            }
            SMValue::Float(val) => {
                if A::is_little_endian() {
                    place.write_all(&val.to_bits().to_le_bytes()).unwrap();
                } else {
                    place.write_all(&val.to_bits().to_be_bytes()).unwrap();
                }
            },
            SMValue::Struct(items) => {
                let start_of_struct = place.position();
                // First compile types of fields
                let mut fields_types = vec![];
                for item in items {
                    fields_types.push(Self::get_item_type(item, m));
                }
                // Then for every field, write the value to where it's supposed to be
                for (n, item) in items.iter().enumerate() {
                    let offset = A::struct_field_offset(&fields_types, n);
                    place.set_position(start_of_struct + offset as u64);
                    Self::write_to_memory::<A>(place, item, m, addresses);
                }
            }
            SMValue::Blob(blob) => {
                place.write_all(&*blob).unwrap();
            }
            SMValue::PtrTo(item_ref) => {
                let address = addresses[item_ref] as u32;
                // TODO: we assume the address is a 32-bit integer, not true for all ABIs
                if A::is_little_endian() {
                    place.write_all(&address.to_le_bytes()).unwrap();
                } else {
                    place.write_all(&address.to_be_bytes()).unwrap();
                }
            }
        }
    }

    fn get_item_type<'m>(item: &SMValue, m: &Module<'m>) -> Ty<'m> {
        match item {
            SMValue::Int8(_, Sign::S) => m.int8t(),
            SMValue::Int8(_, Sign::U) => m.uint8t(),
            SMValue::Int16(_, Sign::S) => m.int16t(),
            SMValue::Int16(_, Sign::U) => m.uint16t(),
            SMValue::Int32(_, Sign::S) => m.int32t(),
            SMValue::Int32(_, Sign::U) => m.uint32t(),
            SMValue::Float(_) => m.float32t(),
            SMValue::Struct(items) => {
                let mut fields = vec![];
                for item in items {
                    fields.push(Self::get_item_type(item, m));
                }
                m.intern_type(Type::Struct { fields })
            }
            // FIXME
            // there's no "array" type or something like this
            // so we simulate it by making a struct full of uint8 types
            SMValue::Blob(blob) => {
                m.intern_type(Type::Struct {
                    fields: vec![m.uint8t(); blob.len()]
                })
            }
            SMValue::PtrTo(_) => m.ptr_t()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{abi::Wasm32Abi};

    use super::*;

    #[test]
    fn staticmem_test() {
        let top = Module::default();

        let mut mem = StaticMemory::new();
        let i1 = mem.add_item(SMItem {
            value: SMValue::Struct(vec![
                SMValue::Int8(64, Sign::S),
                SMValue::Int16(65535, Sign::U),
                SMValue::Blob(Box::new([1, 2, 3, 4, 5, 6, 7, 8]))
            ]),
            mutability: Mutability::Const,
            unique: true
        });
        let i2 = mem.add_item(SMItem {
            value: SMValue::Struct(vec![
                SMValue::Struct(vec![
                    SMValue::PtrTo(i1),
                ]),
                SMValue::Int32(0, Sign::S)
            ]),
            mutability: Mutability::Const,
            unique: true
        });
        mem.add_item(SMItem {
            value: SMValue::Struct(vec![
                SMValue::Int8(1, Sign::U),
                SMValue::PtrTo(i2),
                SMValue::PtrTo(i1),
            ]),
            mutability: Mutability::Const,
            unique: true
        });

        let compiled = CompiledStaticMemory::compile::<Wasm32Abi>(&top, &mem);
        assert_eq!(compiled.buf, vec![
            // The first eight empty bytes
            0, 0, 0, 0, 0, 0, 0, 0,
            64,
            0, // padding between int8 and int16
            255, 255, // 65536 as int16
            1, 2, 3, 4, 5, 6, 7, 8, // the blob
            8, 0, 0, 0, // ptr to start of first struct (the "64")
            0, 0, 0, 0, // 0 as int32
            1,
            0, 0, 0, // padding before ptr
            20, 0, 0, 0, // ptr to start of second struct
            8, 0, 0, 0 // ptr to start of first struct
        ]);
    }
}