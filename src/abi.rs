use crate::ty::{Ty, Type};
use wasm_encoder as wasm;

pub trait Abi {
    /// How value types are represented in the target
    type BackendType;

    /// Compile the frontend Swarm-IR type
    /// to a backend type
    fn compile_type(ty: Ty<'_>) -> Self::BackendType;
    
    /// `sizeof` operation for a type
    fn type_sizeof(ty: Ty<'_>) -> usize;

    /// The alignment of a type
    /// The alignment must be expressed as an exponent of two. Therefore:
    /// 0 => no alignment (1-byte)
    /// 1 => two byte alignment (`short`/`int16` type)
    /// 2 => four byte alignment (`int`/`int32` type)
    /// 3 => eight byte alignment (`long`/`int64` type)
    fn type_alignment(ty: Ty<'_>) -> usize;

    /// Return an offset at which the Nth field
    /// starts inside a struct
    fn struct_field_offset(struct_fields: &[Ty<'_>], field_n: usize) -> usize;
}

pub struct Wasm32Abi {}

impl Abi for Wasm32Abi {
    type BackendType = wasm::ValType;

    fn compile_type(ty: Ty<'_>) -> Self::BackendType {
        match &*ty {
            Type::Int32 | Type::UInt32 | Type::Int16 | Type::UInt16 | Type::Int8 | Type::UInt8 
                => wasm::ValType::I32,
            Type::Float32 => wasm::ValType::F32,
            // Function "types" are actually integer indexes into the global function table
            Type::Func { args: _, ret: _ } => wasm::ValType::I32,
            // TODO: support 64-bit memory and pointers
            Type::Ptr => wasm::ValType::I32,
            // calling compile_type() on a Struct type should never happen in valid code
            Type::Struct { fields: _ } => unreachable!()
        }
    }

    fn type_sizeof(ty: Ty<'_>) -> usize {
        match &*ty {
            Type::Int8  | Type::UInt8  => 1,
            Type::Int16 | Type::UInt16 => 2,
            Type::Int32 | Type::UInt32 => 4,
            Type::Float32 => 4,
            // actually an int32, thus 4
            Type::Func { args:_, ret:_ } => 4,
            // same as above
            Type::Ptr => 4,
            // TODO: cache the results of the struct_calc algorithm, so we don't need to recalculate it every time
            Type::Struct { fields } => struct_calc_algorithm::<Self>(fields).1
        }
    }

    fn type_alignment(ty: Ty<'_>) -> usize {
        match &*ty {
            Type::Int8  | Type::UInt8  => 0,
            Type::Int16 | Type::UInt16 => 1,
            Type::Int32 | Type::UInt32 => 2,
            Type::Float32 => 2,
            Type::Func { args:_, ret:_ } => 2,
            Type::Ptr => 2,
            // TODO: cache the results of the struct_calc algorithm, so we don't need to recalculate it every time
            Type::Struct { fields } => struct_calc_algorithm::<Self>(fields).2
        }
    }

    fn struct_field_offset(struct_fields: &[Ty<'_>], field_n: usize) -> usize {
        // TODO: cache the results of the struct_calc algorithm, so we don't need to recalculate it every time
        struct_calc_algorithm::<Self>(struct_fields).0[field_n]
    }
}

/// The algorithm for calculating struct paddings, size and alignment
/// For the details, see Structs Pt. 1 draft, section "Padding algorithm".
///
/// Returns a vector (field_start_offsets, struct_size, struct_alignment)
fn struct_calc_algorithm<A: Abi>(struct_fields: &[Ty<'_>]) -> (Vec<usize>, usize, usize) {
    let mut field_start_offsets = Vec::new();
    let mut size = 0;
    let mut align = 0; // the alignment is actually one, but we use exponents of two (2**0 = 1)

    for field in struct_fields {
        // we need to convert the field alignment to bytes, because the Abi api uses exponents of two
        let field_alignment = 2_usize.pow(A::type_alignment(*field) as u32);
        // if alignment is not preserved, add padding
        if (size % field_alignment) != 0 {
            let padding_size = field_alignment - (size % field_alignment);
            size += padding_size;
        }
        // now, the field starts
        field_start_offsets.push(size);
        size += A::type_sizeof(*field);
        if A::type_alignment(*field) > align {
            align = A::type_alignment(*field);
        }
    }

    (field_start_offsets, size, align)
}

#[cfg(test)]
mod tests {
    use crate::{abi::{Abi, Wasm32Abi}, module::{Module, WasmModuleConf}, ty::{Ty, Type}};

    #[test]
    pub fn struct_test() {
        // TODO: add more tests
        let mut m = Module::default();

        let struct_t1 = m.intern_type(Type::Struct { fields: vec![
            m.int16t(), /*2-byte padding */ m.int32t(), m.int8t(), m.uint8t()
        ] });

        let struct_t2 = m.intern_type(Type::Struct { fields: vec![
        ] });

        let struct_t3 = m.intern_type(Type::Struct { fields: vec![
            struct_t2, struct_t1, /* 2-byte padding*/ m.float32t(), struct_t1
        ] });

        assert_eq!(Wasm32Abi::type_sizeof(struct_t1), 10);
        assert_eq!(Wasm32Abi::type_alignment(struct_t1), 2); // equal to alignment of int32
        // field 0 (int16) - offset 0
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t1), 0), 0);
        // field 1 (int32) - offset 4
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t1), 1), 4);
        // field 2 (int8) - offset 8
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t1), 2), 8);
        // field 3 (uint8) - offset 9
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t1), 3), 9);

        assert_eq!(Wasm32Abi::type_sizeof(struct_t2), 0);
        assert_eq!(Wasm32Abi::type_alignment(struct_t2), 0);

        assert_eq!(Wasm32Abi::type_sizeof(struct_t3), 26);
        assert_eq!(Wasm32Abi::type_alignment(struct_t3), 2);
        // field 0 (struct2) - offset 0
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t3), 0), 0);
        // field 1 (struct1) - also offset 0
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t3), 1), 0);
        // field 2 (float32) - offset 12
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t3), 2), 12);
        // field 0 (struct2) - offset 16
        assert_eq!(Wasm32Abi::struct_field_offset(helper(&struct_t3), 3), 16);
    }

    fn helper<'a, 'ctx>(ty: &'a Ty<'ctx>) -> &'a [Ty<'ctx>] {
        match ty.as_ref() {
            Type::Struct { fields } => fields,
            _ => unreachable!()
        }
    }
}