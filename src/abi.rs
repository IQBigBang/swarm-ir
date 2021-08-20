use crate::ty::{Ty, Type};
use wasm_encoder as wasm;

pub trait Abi {
    /// How value types are represented in the target
    type BackendType;

    /// Compile the frontend Swarm-IR type
    /// to a backend type
    fn compile_type(ty: Ty<'_>) -> Self::BackendType;
    
    /// `sizeof` operation for backend types
    fn type_sizeof(ty: &Self::BackendType) -> usize;

    /// The alignment of the backend type
    /// The alignment must be expressed as an exponent of two. Therefore:
    /// 0 => no alignment (1-byte)
    /// 1 => two byte alignment (`short`/`int16` type)
    /// 2 => four byte alignment (`int`/`int32` type)
    /// 3 => eight byte alignment (`long`/`int64` type)
    fn type_alignment(ty: &Self::BackendType) -> usize;
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

    fn type_sizeof(ty: &wasm::ValType) -> usize {
        match ty {
            wasm::ValType::I32 => 4,
            wasm::ValType::I64 => 8,
            wasm::ValType::F32 => 4,
            wasm::ValType::F64 => 8,
            _ => unimplemented!()
        }
    }

    fn type_alignment(ty: &wasm::ValType) -> usize {
        match ty {
            wasm::ValType::I32 => 2,
            wasm::ValType::I64 => 3,
            wasm::ValType::F32 => 2,
            wasm::ValType::F64 => 3,
            _ => unimplemented!()
        }
    }
}