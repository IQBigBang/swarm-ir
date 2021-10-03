use std::fmt::Debug;

use libintern::Intern;

use crate::irprint::IRPrint;

#[derive(PartialEq, Eq, Hash)]
pub enum Type<'ctx> {
    /// A signed 8-bit integer
    Int8,
    /// An unsigned 8-bit integer
    UInt8,
    /// A signed 16-bit integer
    Int16,
    /// An unsigned 16-bit integer
    UInt16,
    /// A signed 32-bit integer
    Int32,
    /// An unsigned 32-bit integer
    UInt32,
    Float32,
    Func { args: Vec<Ty<'ctx>>, ret: Vec<Ty<'ctx>> },
    Ptr,
    Struct { fields: Vec<Ty<'ctx>> }
}

impl<'ctx> Type<'ctx> {
    pub fn is_int(&self) -> bool {
        matches!(self, Type::Int32 | Type::UInt32 | Type::Int16 | Type::UInt16 | Type::Int8 | Type::UInt8)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::Float32)
    }

    pub fn is_func(&self) -> bool {
        matches!(self, Type::Func { args: _, ret: _ })
    }

    pub fn is_ptr(&self) -> bool {
        matches!(self, Type::Ptr)
    }

    pub fn is_struct(&self) -> bool {
        matches!(self, Type::Struct { fields: _ })
    }
}

impl Debug for Type<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ir_print(f)
    }
}

pub type Ty<'ctx> = Intern<'ctx, Type<'ctx>>;