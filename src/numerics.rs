//! All integer-manipulation related procedures
//! Includes code for verifying numeric instructions
//! and most importantly for emitting them.

use wasm_encoder::{Instruction, MemArg};

use crate::{abi::Abi, instr::{Cmp, Instr, InstrK}, ty::Ty};

/// The metadata `bws` (BitWidth and Sign) associated
/// with numeric instructions is of this type.
#[derive(Clone, Copy)]
pub(crate) enum BitWidthSign {
    S32,
    U32,
    S16,
    U16,
    S8,
    U8
}

impl BitWidthSign {
    fn is_unsigned(self) -> bool {
        matches!(self, BitWidthSign::U32 | BitWidthSign::U16 | BitWidthSign::U8)
    }
}

/// Emit WASM instructions for numeric IR instructions
///
/// Based on the table(s) from the *Numeric* draft
pub(crate) fn emit_numeric_instr<'a, A: Abi>(kind: &InstrK, bws: BitWidthSign, use_saturating_ftoi: bool) -> Vec<Instruction<'a>> {
    match kind {
        InstrK::IAdd | InstrK::ISub | InstrK::IMul => {
            // these three instruction all compile down to:
            // the-core-instr (iadd,isub,imul)
            // the "and" idiom for small unsigned types
            // the "shift" idiom for small signed types
            let core = match kind {
                InstrK::IAdd => Instruction::I32Add,
                InstrK::ISub => Instruction::I32Sub,
                InstrK::IMul => Instruction::I32Mul,
                _ => unreachable!()
            };
            match bws {
                BitWidthSign::S32 | BitWidthSign::U32 => {
                    // no additional instructions for int32, uint32
                    vec![core]
                }
                BitWidthSign::U16 => and(core, 65536),
                BitWidthSign::S16 => shift(core, 16),
                BitWidthSign::U8 => and(core, 255),
                BitWidthSign::S8 => shift(core, 24),
            }
        },
        InstrK::IDiv => match bws {
            BitWidthSign::U32 | BitWidthSign::U16 | BitWidthSign::U8 => {
                vec![Instruction::I32DivU]
            },
            BitWidthSign::S32 => vec![Instruction::I32DivS],
            BitWidthSign::S16 => shift(Instruction::I32DivS, 16),
            BitWidthSign::S8 => shift(Instruction::I32DivS, 24),
        }
        InstrK::Itof => 
            if bws.is_unsigned() {
                vec![Instruction::F32ConvertI32U]
            } else {
                vec![Instruction::F32ConvertI32S]
            },
        InstrK::Ftoi { int_ty: _ } =>
            if bws.is_unsigned() {
                if use_saturating_ftoi { vec![Instruction::I32TruncSatF32U] }
                else { vec![Instruction::I32TruncF32U] }
            } else if use_saturating_ftoi { 
                vec![Instruction::I32TruncSatF32S] 
            } else { vec![Instruction::I32TruncF32S] }
        ,
        InstrK::ICmp(cmp) => match *cmp {
            Cmp::Eq => vec![Instruction::I32Eq],
            Cmp::Ne => vec![Instruction::I32Neq],
            Cmp::Lt => if bws.is_unsigned() {
                vec![Instruction::I32LtU]
            } else {
                vec![Instruction::I32LtS]
            },
            Cmp::Le => if bws.is_unsigned() {
                vec![Instruction::I32LeU]
            } else {
                vec![Instruction::I32LeS]
            },
            Cmp::Gt => if bws.is_unsigned() {
                vec![Instruction::I32GtU]
            } else {
                vec![Instruction::I32GtS]
            },
            Cmp::Ge => if bws.is_unsigned() {
                vec![Instruction::I32GeU]
            } else {
                vec![Instruction::I32GeS]
            },
        },
        InstrK::Read { ty } => match bws {
            BitWidthSign::U32 | BitWidthSign::S32 => {
                vec![Instruction::I32Load(memarg::<A>(ty))]
            }
            BitWidthSign::U16 => {
                vec![Instruction::I32Load16_U(memarg::<A>(ty))]
            }
            BitWidthSign::S16 => {
                vec![Instruction::I32Load16_S(memarg::<A>(ty))]
            }
            BitWidthSign::U8 => {
                vec![Instruction::I32Load8_U(memarg::<A>(ty))]
            }
            BitWidthSign::S8 => {
                vec![Instruction::I32Load8_S(memarg::<A>(ty))]
            }
        },
        InstrK::Write { ty } => match bws {
            BitWidthSign::U32 | BitWidthSign::S32 => {
                vec![Instruction::I32Store(memarg::<A>(ty))]
            }
            BitWidthSign::U16 | BitWidthSign::S16 => {
                vec![Instruction::I32Store16(memarg::<A>(ty))]
            }
            BitWidthSign::U8 | BitWidthSign::S8 => {
                vec![Instruction::I32Store8(memarg::<A>(ty))]
            }
        }
        InstrK::IConv { target } => match type_to_bws(*target).unwrap() {
            // Conversions to i32, u32 are always no-ops
            BitWidthSign::S32 | BitWidthSign::U32 => vec![],
            BitWidthSign::S16 => match bws {
                BitWidthSign::U16 | BitWidthSign::U32 | BitWidthSign::S32 => {
                    // shift(16)
                    vec![Instruction::I32Const(16), Instruction::I32Shl,
                         Instruction::I32Const(16), Instruction::I32ShrS]
                }
                BitWidthSign::S16 | BitWidthSign::U8 | BitWidthSign::S8 => vec![] // nop
            },
            BitWidthSign::U16 => match bws {
                BitWidthSign::S8 | BitWidthSign::S16 | BitWidthSign::U32 | BitWidthSign::S32 => {
                    // and(65536)
                    vec![Instruction::I32Const(65536), Instruction::I32And]
                }
                BitWidthSign::U8 | BitWidthSign::U16 => vec![] // nop
            },
            BitWidthSign::S8 => match bws {
                BitWidthSign::S32 | BitWidthSign::U32 | BitWidthSign::S16 | BitWidthSign::U16 | BitWidthSign::U8 => {
                    // shift(24)
                    vec![Instruction::I32Const(24), Instruction::I32Shl,
                         Instruction::I32Const(24), Instruction::I32ShrS]
                }
                BitWidthSign::S8 => vec![], // nop
            },
            BitWidthSign::U8 => match bws {
                BitWidthSign::S32 | BitWidthSign::U32 | BitWidthSign::S16 | BitWidthSign::U16 | BitWidthSign::S8 => {
                    // and(255)
                    vec![Instruction::I32Const(255), Instruction::I32And]
                }
                BitWidthSign::U8 => vec![], // nop
            },
        }
        _ => unreachable!()
    }
}

fn memarg<A: Abi>(ty: &Ty<'_>) -> MemArg {
    MemArg {
        offset: 0,
        align: A::type_alignment(*ty) as u32,
        memory_index: 0,
    }
}

/// Helper function. Emits the very common `and` idiom:
/// * `i32.const N`
/// * `i32.and`
fn and(inner: Instruction, n: i32) -> Vec<Instruction> {
    vec![
        inner,
        Instruction::I32Const(n),
        Instruction::I32And
    ]
}

/// Helper function. Emits the very common `shift` idiom:
/// * `i32.const N`
/// * `i32.shl`
/// * `i32.const N`
/// * `i32.shr_s`
fn shift(inner: Instruction, n: i32) -> Vec<Instruction> {
    vec![
        inner,
        Instruction::I32Const(n),
        Instruction::I32Shl,
        Instruction::I32Const(n),
        Instruction::I32ShrS
    ]
}

/// Returns true if the instructions has to have numeric metadata
/// attached to it.
///
/// Some instructions (namely Ftoi, Read, Write) require the integer bitwidth & sign
/// but can get it from their explicit type arguments and therefore do NOT require
#[allow(unused)]
pub(crate) fn instr_needs_numeric_metadata(i: &Instr<'_>) -> bool {
    matches!(&i.kind, 
        InstrK::IAdd | InstrK::ISub | InstrK::IMul | InstrK::IDiv |
        InstrK::Itof | InstrK::ICmp(_) | InstrK::IConv { target: _ })
}

/// Returns the BWS descriptor of a type.
/// Returns None if it's not an integer type
pub(crate) fn type_to_bws(t: Ty<'_>) -> Option<BitWidthSign> {
    match &*t {
        crate::ty::Type::Int32 => Some(BitWidthSign::S32),
        crate::ty::Type::UInt32 => Some(BitWidthSign::U32),
        crate::ty::Type::Int16 => Some(BitWidthSign::S16),
        crate::ty::Type::UInt16 => Some(BitWidthSign::U16),
        crate::ty::Type::Int8 => Some(BitWidthSign::S8),
        crate::ty::Type::UInt8 => Some(BitWidthSign::U8),
        _ => None
    }
}

pub(crate) fn do_int_types_match(l: Ty<'_>, r: Ty<'_>) -> bool {
    debug_assert!(l.is_int() && r.is_int());
    matches!((&*l, &*r), 
        (crate::ty::Type::Int32, crate::ty::Type::Int32)   | 
        (crate::ty::Type::UInt32, crate::ty::Type::UInt32) |
        (crate::ty::Type::Int16, crate::ty::Type::Int16)   |
        (crate::ty::Type::UInt16, crate::ty::Type::UInt16) |
        (crate::ty::Type::Int8, crate::ty::Type::Int8)     |
        (crate::ty::Type::UInt8, crate::ty::Type::UInt8))
}