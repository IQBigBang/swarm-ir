//! Intrinsics are special instructions used by the SwarmIR optimizations.
//!
//! They are considered a private implementation detail and therefore not exposed
//! in the public API.
//!

use crate::ty::Ty;

#[repr(transparent)]
#[derive(PartialEq, Debug, Clone)]
pub struct Intrinsic<'ctx>(pub(crate) Intrinsics<'ctx>);

// The intrinsics are actually a private implementation detail and not exposed to the user
#[derive(PartialEq, Debug, Clone)]
pub(crate) enum Intrinsics<'ctx> {
}