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
}

pub struct Wasm32Abi {}

impl Abi for Wasm32Abi {
    type BackendType = wasm::ValType;

    fn compile_type(ty: Ty<'_>) -> Self::BackendType {
        match &*ty {
            Type::Int32 => wasm::ValType::I32,
            Type::Float32 => wasm::ValType::F32,
            // Function "types" are actually integer indexes into the global function table
            Type::Func { args: _, ret: _ } => wasm::ValType::I32,
            // TODO: support 64-bit memory and pointers
            Type::Ptr => wasm::ValType::I32
        }
    }

    fn type_sizeof(ty: Ty<'_>) -> usize {
        match &*ty {
            Type::Int32 => 4,
            Type::Float32 => 4,
            // actually an int32, thus 4
            Type::Func { args:_, ret:_ } => 4,
            // same as above
            Type::Ptr => 4,
        }
    }

    fn type_alignment(ty: Ty<'_>) -> usize {
        match &*ty {
            Type::Int32 => 2,
            Type::Float32 => 2,
            Type::Func { args:_, ret:_ } => 2,
            Type::Ptr => 2,
        }
    }
}