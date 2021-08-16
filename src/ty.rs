use std::fmt::Debug;

use libintern::Intern;

use crate::irprint::IRPrint;

#[derive(PartialEq, Eq, Hash)]
pub enum Type<'ctx> {
    Int32,
    Float32,
    Func { args: Vec<Ty<'ctx>>, ret: Vec<Ty<'ctx>> }
}

impl<'ctx> Type<'ctx> {
    pub fn is_int(&self) -> bool {
        matches!(self, Type::Int32)
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::Float32)
    }

    pub fn is_func(&self) -> bool {
        matches!(self, Type::Func { args: _, ret: _ })
    }
}

impl Debug for Type<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ir_print(f)
    }
}

pub type Ty<'ctx> = Intern<'ctx, Type<'ctx>>;