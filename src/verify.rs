use std::collections::HashMap;

use crate::{instr::{Instr, InstrK}, pass::{MutableFunctionPass}, ty::{Ty, Type}};

pub struct Verifier {}

pub struct VerifierMutInfo<'ctx> {
    /// Types of the functions in CallIndirect instructions
    call_indirect_function_types: HashMap<usize, Ty<'ctx>>,
    /// Types of the `from`s of BitCast instructions
    bitcast_source_types: HashMap<usize, Ty<'ctx>>
}

impl<'ctx> MutableFunctionPass<'ctx> for Verifier {
    type Error = VerifyError<'ctx>;
    type MutationInfo = VerifierMutInfo<'ctx>;

    fn visit_function(
        &mut self, 
        module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<VerifierMutInfo<'ctx>, Self::Error> {
        
        // We simulate and record the function stack types
        let mut stack = Vec::new();

        let mut call_indirect_function_types = HashMap::new();
        let mut bitcast_source_types = HashMap::new();

        for (i, instr) in function.body().body.iter().enumerate() {
            match &instr.kind {
                InstrK::LdInt(_) => stack.push(module.int32t()),
                InstrK::LdFloat(_) => stack.push(module.float32t()),
                InstrK::IAdd | InstrK::ISub | InstrK::IMul | InstrK::IDiv | InstrK::ICmp(_) => {
                    let lhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    let rhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;

                    match (&*lhs, &*rhs) {
                        (Type::Int32, Type::Int32) => stack.push(module.int32t()),
                        (Type::Int32, _) => return Err(VerifyError::InvalidType { 
                            expected: module.int32t(),
                            actual: rhs,
                            reason: "Integer arithmetic operation"
                        }),
                        _ => return Err(VerifyError::InvalidType { 
                            expected: module.int32t(),
                            actual: lhs,
                            reason: "Integer arithmetic operation"
                        })
                    }
                },
                InstrK::FAdd | InstrK::FSub | InstrK::FMul | InstrK::FDiv => {
                    let lhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    let rhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;

                    match (&*lhs, &*rhs) {
                        (Type::Float32, Type::Float32) => stack.push(module.float32t()),
                        (Type::Float32, _) => return Err(VerifyError::InvalidType { 
                            expected: module.float32t(),
                            actual: rhs,
                            reason: "Integer arithmetic operation"
                        }),
                        _ => return Err(VerifyError::InvalidType { 
                            expected: module.float32t(),
                            actual: lhs,
                            reason: "Integer arithmetic operation"
                        })
                    }
                },
                /* FCmp is different, because its result is an integer, not a floating point */
                InstrK::FCmp(_) => {
                    let lhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    let rhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    match (&*lhs, &*rhs) {
                        (Type::Float32, Type::Float32) => stack.push(module.int32t()),
                        (Type::Float32, _) => return Err(VerifyError::InvalidType { 
                            expected: module.float32t(),
                            actual: rhs,
                            reason: "Integer arithmetic operation"
                        }),
                        _ => return Err(VerifyError::InvalidType { 
                            expected: module.float32t(),
                            actual: lhs,
                            reason: "Integer arithmetic operation"
                        })
                    }
                }
                InstrK::Itof => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !val.is_int() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.int32t(),
                            actual: val,
                            reason: "Itof instruction"
                        })
                    }
                    stack.push(module.float32t())
                }
                InstrK::Ftoi => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !val.is_float() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.float32t(),
                            actual: val,
                            reason: "Itof instruction"
                        })
                    }
                    stack.push(module.int32t())
                }
                /*InstrK::Return => {
                    //let val = stack.pop().ok_or(VerifyError::StackUnderflow)?
                    /* FIXME - "Return" changes the block, it does not actually return from a function */
                    if function.ret_tys() != &stack {
                        return Err(VerifyError::GeneralError);
                        /*return Err(VerifyError::InvalidType { 
                            expected: function.ret_ty(),
                            actual: val,
                            reason: "Function return"
                        })*/
                    }
                },*/
                InstrK::CallDirect { func_name } => {
                    match module.get_function(func_name) {
                        None => return Err(VerifyError::UndefinedFunctionCall {
                            func_name: func_name.to_owned()
                        }),
                        Some(func) => {
                            // Check the argument types
                            for &arg in func.arg_tys() {
                                let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                                if arg != val {
                                    return Err(VerifyError::InvalidType { 
                                        expected: arg,
                                        actual: val,
                                        reason: "Function call argument"
                                    })
                                }
                            }
                            // Add values of the return types
                            stack.extend(func.ret_tys());
                        }
                    }
                }
                InstrK::LdLocal { idx } => {
                    let loc_ty = function.local_ty(*idx).ok_or(VerifyError::OutOfBoundsLocalIndex)?;
                    stack.push(loc_ty);
                },
                InstrK::StLocal { idx } => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    let loc_ty = function.local_ty(*idx).ok_or(VerifyError::OutOfBoundsLocalIndex)?;
                    if loc_ty != val {
                        return Err(VerifyError::InvalidType {
                            expected: loc_ty,
                            actual: val,
                            reason: "Local store"
                        })
                    }
                },
                InstrK::LdGlobalFunc { func_name } => {
                    match module.get_function(func_name) {
                        None => return Err(VerifyError::UndefinedFunctionCall {
                            func_name: func_name.to_owned()
                        }),
                        Some(func) => {
                            stack.push(func.ty());
                        }
                    }
                },
                InstrK::CallIndirect => {
                    let func = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    match &*func {
                        Type::Func { args, ret } => {
                            // Check the argument types
                            for &arg in args {
                                let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                                if arg != val {
                                    return Err(VerifyError::InvalidType { 
                                        expected: arg,
                                        actual: val,
                                        reason: "Indirect function call argument"
                                    })
                                }
                            }
                            // Add values of return types
                            stack.extend(ret);
                        },
                        _ => return Err(VerifyError::InvalidTypeCallIndirect)
                    }

                    call_indirect_function_types.insert(i, func);
                },
                InstrK::Bitcast { target } => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    match (&*val, &**target) {
                        (Type::Int32, Type::Int32) | (Type::Int32, Type::Float32) |
                        (Type::Float32, Type::Int32) | (Type::Float32, Type::Float32) |
                        (Type::Int32, Type::Func { args: _, ret: _ }) | (Type::Float32, Type::Func { args: _, ret: _ }) |
                        (Type::Func { args: _, ret: _ }, Type::Int32) | (Type::Func { args: _, ret: _ }, Type::Float32) |
                        (Type::Func { args: _, ret: _ }, Type::Func { args: _, ret: _ }) => {
                            // All these cases are OK!
                            stack.push(*target);
                        }
                    }
                    bitcast_source_types.insert(i, val);
                }
                InstrK::End => {
                    if i != function.body().body.len() - 1 {
                        return Err(VerifyError::UnexpectedEndOfBlock)
                    }
                }
            }
        }

        // also make sure the block ends with an End instruction
        match function.body().body.last() {
            Some(Instr { meta: _, kind: InstrK::End }) => {},
            _ => return Err(VerifyError::UnexpectedEndOfBlock)
        }

        Ok(VerifierMutInfo { call_indirect_function_types, bitcast_source_types })
    }

    fn mutate_function(
        &mut self,
        function: &mut crate::instr::Function<'ctx>,
        info: VerifierMutInfo<'ctx>) -> Result<(), Self::Error> {
        
        for (i, instr) in function.body_mut().body.iter_mut().enumerate() {
            if info.call_indirect_function_types.contains_key(&i) {
                let function_ty = info.call_indirect_function_types[&i];

                debug_assert!(matches!(instr.kind, InstrK::CallIndirect));

                instr.meta.insert_ty("ty", function_ty)
            }
            
            if info.bitcast_source_types.contains_key(&i) {
                let source_ty = info.bitcast_source_types[&i];

                debug_assert!(matches!(instr.kind, InstrK::Bitcast { target: _ }));

                instr.meta.insert_ty("from", source_ty)
            }
        }

        Ok(())
    }
    
}

#[derive(Debug)]
pub enum VerifyError<'ctx> {
    GeneralError,
    StackUnderflow,
    InvalidType { expected: Ty<'ctx>, actual: Ty<'ctx>, reason: &'static str },
    UnexpectedEndOfBlock,
    UndefinedFunctionCall { func_name: String },
    OutOfBoundsLocalIndex,
    InvalidTypeCallIndirect
}