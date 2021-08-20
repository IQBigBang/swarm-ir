use std::collections::HashMap;

use crate::{instr::{BlockId, Instr, InstrBlock, InstrK}, pass::{MutableFunctionPass}, ty::{Ty, Type}};

pub struct Verifier {}

pub struct VerifierMutInfo<'ctx> {
    /// Types of the functions in CallIndirect instructions
    call_indirect_function_types: HashMap<(BlockId, usize), Ty<'ctx>>,
    /// Types of the `from`s of BitCast instructions
    bitcast_source_types: HashMap<(BlockId, usize), Ty<'ctx>>
}

impl<'ctx> Verifier {
    fn verify_block(
        &self,
        out_info: &mut VerifierMutInfo<'ctx>,
        this_block_id: BlockId,
        module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>,
        block: &InstrBlock<'ctx>
    ) -> Result<(), VerifyError<'ctx>> {

        // We simulate and record the function stack types
        // Every block starts with an empty stack (values can't be passed to blocks)
        let mut stack = Vec::new();

        for (i, instr) in block.body.iter().enumerate() {
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

                    out_info.call_indirect_function_types.insert((this_block_id, i), func);
                },
                InstrK::Bitcast { target } => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    #[allow(clippy::match_single_binding)]
                    match (&*val, &**target) {
                        _ => {
                            // All cases are OK, this might not be true in the future
                            // that's why this match statement is here
                            stack.push(*target);
                        }
                    }
                    out_info.bitcast_source_types.insert((this_block_id, i), val);
                }
                InstrK::End => {
                    if i != block.body.len() - 1 {
                        return Err(VerifyError::UnexpectedEndOfBlock)
                    }
                }
                InstrK::IfElse { then, r#else } => {
                    // the condition
                    let cond = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !cond.is_int() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.int32t(),
                            actual: cond,
                            reason: "If condition"
                        })
                    }
                    // verify the block types are the same
                    let then_block_returns = 
                        function.get_block(*then)
                        .ok_or(VerifyError::InvalidBlockId)?
                        .returns();
                    match r#else {
                        Some(i) => {
                            let else_block_returns = 
                                function.get_block(*i)
                                .ok_or(VerifyError::InvalidBlockId)?
                                .returns();
                            
                            if then_block_returns != else_block_returns {
                                return Err(VerifyError::InvalidBlockType {
                                    block: *i,
                                    expected: then_block_returns.clone(),
                                    actual: else_block_returns.clone()
                                })
                            }
                        }
                        None => {
                            /* if the else block is None, its' return is [] */
                            if !then_block_returns.is_empty() {
                                return Err(VerifyError::InvalidBlockType {
                                    block: *then,
                                    expected: vec![],
                                    actual: then_block_returns.clone()
                                })
                            }
                        }
                    }
                    // push the values onto the stack
                    stack.extend_from_slice(then_block_returns);
                }
                InstrK::Read { ty } => {
                    if ty.is_struct() {
                        return Err(VerifyError::UnexpectedStructType {
                            r#where: "Read instruction"
                        })
                    }
                    let ptr = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !ptr.is_ptr() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.ptr_t(),
                            actual: ptr,
                            reason: "Read instruction"
                        })
                    }
                    stack.push(*ty);
                }
                InstrK::Write { ty } => {
                    if ty.is_struct() {
                        return Err(VerifyError::UnexpectedStructType {
                            r#where: "Read instruction"
                        })
                    }
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if val != *ty {
                        return Err(VerifyError::InvalidType {
                            expected: *ty,
                            actual: val,
                            reason: "Write instruction"
                        })
                    }
                    let ptr = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !ptr.is_ptr() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.ptr_t(),
                            actual: ptr,
                            reason: "Write instruction"
                        })
                    }
                }
                InstrK::Offset { ty } => {
                    // Offset requires an integer and a pointer, pushes a pointer
                    let num = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !num.is_int() {
                        return Err(VerifyError::InvalidType {
                            expected: module.int32t(),
                            actual: num,
                            reason: "Offset instruction"
                        })
                    }
                    let ptr = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !ptr.is_ptr() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.ptr_t(),
                            actual: ptr,
                            reason: "Offset instruction"
                        })
                    }
                    stack.push(module.ptr_t());
                }
            }
        }

        // also make sure the block ends with an End instruction
        match block.body.last() {
            Some(Instr { meta: _, kind: InstrK::End }) => {},
            _ => return Err(VerifyError::UnexpectedEndOfBlock)
        }

        // at the end of the block, check if the types left on the stack
        // agree with the block's type
        if !stack.iter()
            .zip(block.returns().iter())
            .all(|(t1, t2)| *t1 == *t2) {
            // if not all types are equal =>
            return Err(VerifyError::InvalidBlockType {
                block: this_block_id,
                expected: block.returns().clone(),
                actual: stack
            })
        }

        Ok(())
    }

    /// Ensure that there are no arguments, return values, locals or block types with a bare `struct` type
    fn verify_no_struct_types(&self, function: &crate::instr::Function<'ctx>) -> Result<(), VerifyError<'ctx>> {
        for ty in function.all_locals_ty() {
            if ty.is_struct() {
                return Err(VerifyError::UnexpectedStructType { r#where: "Function local" })
            }
        }
        for ty in function.ret_tys() {
            if ty.is_struct() {
                return Err(VerifyError::UnexpectedStructType { r#where: "Function return value" })
            }
        }
        for block in function.blocks_iter() {
            for ty in block.returns() {
                if ty.is_struct() {
                    return Err(VerifyError::UnexpectedStructType { r#where: "Block return value" })
                }
            }
        }
        Ok(())
    }
}

impl<'ctx> MutableFunctionPass<'ctx> for Verifier {
    type Error = VerifyError<'ctx>;
    type MutationInfo = VerifierMutInfo<'ctx>;

    fn visit_function(
        &mut self, 
        module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<VerifierMutInfo<'ctx>, Self::Error> {

        let mut info = VerifierMutInfo {
            call_indirect_function_types: HashMap::new(),
            bitcast_source_types: HashMap::new()
        };

        // do this before verifying the blocks themselves
        self.verify_no_struct_types(function)?;
        
        for block in function.blocks_iter() {
            self.verify_block(
                &mut info,
                block.idx,
                module,
                function,
                block
            )?
        }

        Ok(info)

        // TODO: verify the main block's type is equal to the function's type
    }

    fn mutate_function(
        &mut self,
        function: &mut crate::instr::Function<'ctx>,
        info: VerifierMutInfo<'ctx>) -> Result<(), Self::Error> {
        
        for block in function.blocks_iter_mut() {
            let block_id = block.idx;

            for (i, instr) in block.body.iter_mut().enumerate() {
                let key = (block_id, i);

                if info.call_indirect_function_types.contains_key(&key) {
                    let function_ty = info.call_indirect_function_types[&key];
    
                    debug_assert!(matches!(instr.kind, InstrK::CallIndirect));
    
                    instr.meta.insert_ty("ty", function_ty)
                }
                
                if info.bitcast_source_types.contains_key(&key) {
                    let source_ty = info.bitcast_source_types[&key];
    
                    debug_assert!(matches!(instr.kind, InstrK::Bitcast { target: _ }));
    
                    instr.meta.insert_ty("from", source_ty)
                }
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
    InvalidTypeCallIndirect,
    InvalidBlockType { block: BlockId, expected: Vec<Ty<'ctx>>, actual: Vec<Ty<'ctx>> },
    InvalidBlockId,
    UnexpectedStructType { r#where: &'static str },
}