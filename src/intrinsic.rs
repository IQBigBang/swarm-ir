//! Intrinsics are special instructions used by the SwarmIR optimizations.
//!
//! They are considered a private implementation detail and therefore not exposed
//! in the public API.
//!
//! The list of currently used instrinsic instructions is: (for orientation)
//! * ReadOffset - merges a GetElementPtr and Read instruction into one for more efficient compilation

use crate::ty::Ty;

#[repr(transparent)]
#[derive(PartialEq, Debug, Clone)]
pub struct Intrinsic<'ctx>(pub(crate) Intrinsics<'ctx>);

// The intrinsics are actually a private implementation detail and not exposed to the user
#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Intrinsics<'ctx> {
    // WebAssembly allows for Read/Write (resp. load/store) instructions to specify an offset.
    // For simplicity, SwarmIR does not offer memory instructions with an explicit offset.
    // These two instructions replace the GetFieldPtr & Read/Write combination
    /// Pop a memory address and read a value of type [`ty`] at memory_address + [`offset`]
    ReadAtOffset{ offset: usize, ty: Ty<'ctx> },
    /// Pop a value and a memory address and write the value of type [`ty`] at memory_address + [`offset`]
    WriteAtOffset{ offset: usize, ty: Ty<'ctx> },
}