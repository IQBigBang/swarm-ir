use std::collections::HashMap;

use crate::{instr::{BlockId, InstrBlock, InstrK}, module::Functional, numerics::{BitWidthSign, do_int_types_match, type_to_bws}, pass::MutableFunctionPass, ty::{Ty, Type}};

pub struct Verifier {}

pub struct VerifierMutInfo<'ctx> {
    /// Types of the functions in CallIndirect instructions
    call_indirect_function_types: HashMap<(BlockId, usize), Ty<'ctx>>,
    /// Types of the `from`s of BitCast instructions
    bitcast_source_types: HashMap<(BlockId, usize), Ty<'ctx>>,
    numeric_instrs_data: HashMap<(BlockId, usize), BitWidthSign>
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
                InstrK::LdInt(val, ty) => {
                    if !ty.is_int() {
                        return Err(VerifyError::InvalidType {
                            expected: module.int32t(),
                            actual: *ty,
                            reason: "LdInt instruction"
                        })
                    }
                    // Also verify the integer doesn't overflow the type
                    match &**ty {
                        Type::Int8 => if (*val as i32 > i8::MAX as i32) || ((*val as i32) < i8::MIN as i32) {
                            return Err(VerifyError::ConstIntOverflow { value: *val, ty: *ty })
                        },
                        Type::UInt8 => if *val as i32 > u8::MAX as i32 {
                            return Err(VerifyError::ConstIntOverflow { value: *val, ty: *ty })
                        },
                        Type::Int16 => if (*val as i32 > i16::MAX as i32) || ((*val as i32) < i16::MIN as i32) {
                            return Err(VerifyError::ConstIntOverflow { value: *val, ty: *ty })
                        },
                        Type::UInt16 => if *val as i32 > u16::MAX as i32 {
                            return Err(VerifyError::ConstIntOverflow { value: *val, ty: *ty })
                        },
                        Type::Int32 | Type::UInt32 => { /* can't overflow because IT IS a u32 */ },
                        _ => unreachable!()
                    }
                    stack.push(*ty);
                }
                InstrK::LdFloat(_) => stack.push(module.float32t()),
                InstrK::IAdd | InstrK::ISub | InstrK::IMul | InstrK::IDiv | InstrK::ICmp(_) => {
                    let lhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    let rhs = stack.pop().ok_or(VerifyError::StackUnderflow)?;

                    if !lhs.is_int() {
                        return Err(VerifyError::InvalidType {
                            expected: if rhs.is_int() { rhs } else { module.int32t() /* default to i32 */ },
                            actual: lhs,
                            reason: "Integer numeric operation"
                        })
                    } else if !rhs.is_int() {
                        return Err(VerifyError::InvalidType {
                            expected: if lhs.is_int() { lhs } else { module.int32t() /* default to i32 */ },
                            actual: rhs,
                            reason: "Integer numeric operation"
                        })
                    }
                    // Now they're both surely integers
                    if !do_int_types_match(lhs, rhs) {
                        return Err(VerifyError::IntegerSizeMismatch {left: lhs, right: rhs})
                    }

                    // The metadata stores the operand type, not necessarily the result type (see below)
                    out_info.numeric_instrs_data.insert((block.idx, i), type_to_bws(lhs).unwrap());
                    
                    let result_ty = if let InstrK::ICmp(_) = &instr.kind {
                        // ICmp returns a "boolean", which is always an int32
                        module.int32t()
                    } else {
                        // The lhs and rhs types are the same as the result
                        lhs
                    };
                    stack.push(result_ty);
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
                    // Save integer numeric metadata
                    out_info.numeric_instrs_data.insert((block.idx, i), type_to_bws(val).unwrap());
                    stack.push(module.float32t())
                }
                InstrK::Ftoi { int_ty } => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !val.is_float() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.float32t(),
                            actual: val,
                            reason: "Itof instruction"
                        })
                    }
                    if !int_ty.is_int() {
                        return Err(VerifyError::InvalidType {
                            expected: module.int32t(), // default to int32
                            actual: *int_ty,
                            reason: "Itof instruction target type"
                        })
                    }
                    // Save integer numeric metadata
                    out_info.numeric_instrs_data.insert((block.idx, i), type_to_bws(*int_ty).unwrap());
                    stack.push(*int_ty)
                }
                InstrK::IConv { target } => {
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !val.is_int() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.int32t(),
                            actual: val,
                            reason: "IConv instruction"
                        })
                    }
                    if !target.is_int() {
                        return Err(VerifyError::InvalidType {
                            expected: module.int32t(), // default to int32
                            actual: *target,
                            reason: "IConv instruction target type"
                        })
                    }
                    // Save integer numeric metadata
                    // IConv needs to save its source type, the target type is explicitly specified
                    out_info.numeric_instrs_data.insert((block.idx, i), type_to_bws(val).unwrap());
                    stack.push(*target)
                }
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
                    // Arguments cannot be mutated
                    if function.is_local_an_arg(*idx) {
                        return Err(VerifyError::ArgumentStore {
                            idx: *idx
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
                InstrK::Offset { ty: _ } => {
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
                InstrK::GetFieldPtr { struct_ty, field_idx } => {
                    // Verify the type is, in fact, a struct type
                    if !struct_ty.is_struct() {
                        return Err(VerifyError::GetFieldPtrExpectedStructType)
                    }
                    // Verify the index doesn't point out of bounds
                    let struct_field_count = match &**struct_ty {
                        Type::Struct { fields } => fields.len(),
                        _ => unreachable!()
                    };
                    if *field_idx > struct_field_count {
                        return Err(VerifyError::OutOfBoundsStructIndex)
                    }
                    // Verify there's a pointer type on stack
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
                InstrK::Discard => {
                    if stack.pop().is_none() {
                        return Err(VerifyError::StackUnderflow);
                    }
                }
                InstrK::Return => {
                    if stack.len() != function.ret_count() {
                        return Err(VerifyError::StackUnderflow); // TODO return correct error
                    }
                    for i in (stack.len()-1)..=0 {
                        let on_stack_type = stack.pop().unwrap();
                        if on_stack_type != function.ret_tys()[i] {
                            return Err(VerifyError::InvalidType {
                                expected: function.ret_tys()[i],
                                actual: on_stack_type,
                                reason: "Return instruction",
                            })
                        }
                    }
                }
                InstrK::MemorySize => {
                    // just pushes an int
                    stack.push(module.int32t())
                }
                InstrK::MemoryGrow => {
                    // pops an int and pushes it again
                    let val = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    if !val.is_int() {
                        return Err(VerifyError::InvalidType { 
                            expected: module.int32t(),
                            actual: val,
                            reason: "MemoryGrow instruction"
                        })
                    }
                    stack.push(val); // it's an int
                }
                InstrK::LdGlobal(name) => {
                    let g = module.get_global(name).ok_or_else(|| VerifyError::UndefinedGlobal { name: name.clone() })?;
                    stack.push(g.ty);
                }
                InstrK::StGlobal(name) => {
                    let value = stack.pop().ok_or(VerifyError::StackUnderflow)?;
                    let g = module.get_global(name).ok_or_else(|| VerifyError::UndefinedGlobal { name: name.clone() })?;
                    if value != g.ty {
                        return Err(VerifyError::InvalidType {
                            expected: g.ty,
                            actual: value,
                            reason: "StGlobal instruction"
                        })
                    }
                }
                InstrK::Fail => {
                    // This instruction stops execution so anything
                    // after it is ignored
                    return Ok(())
                }
                InstrK::Loop(body) => {
                    // Verify that the body block's type is () -> ()
                    let body_block_returns = function.get_block(*body)
                        .ok_or(VerifyError::InvalidBlockId)?.returns();
                    if !body_block_returns.is_empty() {
                        return Err(VerifyError::InvalidBlockType {
                            block: *body,
                            expected: vec![],
                            actual: body_block_returns.clone()
                        })
                    }
                }
                InstrK::Intrinsic(_) => {
                    // As of now, all intrinsics are inserted with optimizations
                    // therefore they're not present at verification
                    unreachable!()
                }
            }
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
            bitcast_source_types: HashMap::new(),
            numeric_instrs_data: HashMap::new()
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
    
                    instr.meta.insert_ty(key!("ty"), function_ty)
                }
                
                if info.bitcast_source_types.contains_key(&key) {
                    let source_ty = info.bitcast_source_types[&key];
    
                    debug_assert!(matches!(instr.kind, InstrK::Bitcast { target: _ }));
    
                    instr.meta.insert_ty(key!("from"), source_ty)
                }

                if info.numeric_instrs_data.contains_key(&key) {
                    let bws = info.numeric_instrs_data[&key];

                    instr.meta.insert(key!("bws"), bws)
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
    UndefinedFunctionCall { func_name: String },
    OutOfBoundsLocalIndex,
    InvalidTypeCallIndirect,
    InvalidBlockType { block: BlockId, expected: Vec<Ty<'ctx>>, actual: Vec<Ty<'ctx>> },
    InvalidBlockId,
    UnexpectedStructType { r#where: &'static str },
    GetFieldPtrExpectedStructType,
    OutOfBoundsStructIndex,
    UndefinedGlobal { name: String },
    IntegerSizeMismatch { left: Ty<'ctx>, right: Ty<'ctx>},
    ConstIntOverflow { value: u32, ty: Ty<'ctx> },
    ArgumentStore { idx: usize }
}