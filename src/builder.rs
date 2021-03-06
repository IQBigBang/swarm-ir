//! Function builder helps with creating valid IR.

use std::collections::HashMap;

use crate::{instr::{BlockId, BlockTag, Cmp, Function, Instr, InstrBlock, InstrK}, metadata::Metadata, module::Module, staticmem::SMItemRef, ty::{Ty, Type}};

pub struct FunctionBuilder<'ctx> {
    blocks: HashMap<BlockId, (Vec<Ty<'ctx>>, Vec<Instr<'ctx>>, BlockTag)>,
    next_block_id: usize,
    /// The index of the block currently being modified
    current_block: usize,
    /// A vector of locals INCLUDING the arguments
    locals: Vec<Ty<'ctx>>,
    /// Argument count
    argc: usize,
    /// Return types
    ret: Vec<Ty<'ctx>>,
    /// The function name
    fname: String
}

impl<'ctx> FunctionBuilder<'ctx> {
    pub fn new(
        func_name: String, 
        arguments: impl IntoIterator<Item = Ty<'ctx>>, 
        returns: impl IntoIterator<Item = Ty<'ctx>>) -> Self {

        let returns: Vec<_> = returns.into_iter().collect();

        // The type of the block is what values it returns
        // The main "entry" block returns the same values the function does
        let entry_block = (returns.clone(), vec![], BlockTag::Main);

        let locals: Vec<_> = arguments.into_iter().collect();

        FunctionBuilder {
            blocks: {
                let mut h = HashMap::new();
                h.insert(0.into(), entry_block);
                h
            },
            next_block_id: 1,
            current_block: 0,
            argc: locals.len(),
            locals,
            ret: returns,
            fname: func_name,
        }
    }

    /// Get reference to an Nth argument
    pub fn get_arg(&self, arg_index: usize) -> LocalRef {
        assert!(arg_index < self.argc);
        LocalRef(arg_index)
    }

    pub fn new_local(&mut self, ty: Ty<'ctx>) -> LocalRef {
        self.locals.push(ty);
        LocalRef(self.locals.len() - 1)
    }

    pub fn new_block(&mut self, returns: impl IntoIterator<Item = Ty<'ctx>>, tag: BlockTag) -> BlockId {
        let new_block_id = self.next_block_id.into();
        self.next_block_id += 1;
        let returns: Vec<_> = returns.into_iter().collect();
        let new_block = (returns, vec![], tag);
        self.blocks.insert(new_block_id, new_block);
        new_block_id
    }

    pub fn switch_block(&mut self, new_current_block: BlockId) {
        assert!(self.blocks.contains_key(&new_current_block));
        self.current_block = new_current_block.into();
    }

    pub fn get_current_block(&self) -> BlockId {
        self.current_block.into()
    }

    /// Finish building the current function and add it to the module
    pub fn finish(self, module: &mut Module<'ctx>) {
        // Build the blocks
        let mut blocks = HashMap::new();
        for (id, (returns, mut instrs, tag)) in self.blocks {
            let block_ty = module.intern_type(Type::Func { args: vec![], ret: returns });
            let mut block = InstrBlock::new(id, block_ty, tag);
            block.body.append(&mut instrs);

            let x = blocks.insert(id, block);
            debug_assert!(x.is_none()); // In debug builds, assert there are no two blocks with the same ID
        }

        let func_ty = module.intern_type(
            Type::Func { args: self.locals[0..self.argc].iter().copied().collect(), ret: self.ret }
        );
        let func = Function::new(
            self.fname,
            func_ty,
            blocks,
            self.locals
        );
        module.add_function(func);
    }
}

impl<'ctx> InstrBuilder<'ctx> for FunctionBuilder<'ctx> {
    fn instr(&mut self, i: InstrK<'ctx>) {
        let curr_block = self.current_block.into();
        self.blocks.get_mut(&curr_block).unwrap().1.push(
            Instr { kind: i, meta: Metadata::new() }
        );
    }
}

pub trait InstrBuilder<'ctx> {
    fn instr(&mut self, i: InstrK<'ctx>);

    fn i_ld_int(&mut self, val: u32, int_ty: Ty<'ctx>) { self.instr(InstrK::LdInt(val, int_ty)) }
    fn i_ld_float(&mut self, val: f32) { self.instr(InstrK::LdFloat(val)) }
    fn i_iadd(&mut self) { self.instr(InstrK::IAdd) }
    fn i_isub(&mut self) { self.instr(InstrK::ISub) }
    fn i_imul(&mut self) { self.instr(InstrK::IMul) }
    fn i_idiv(&mut self) { self.instr(InstrK::IDiv) }
    fn i_fadd(&mut self) { self.instr(InstrK::FAdd) }
    fn i_fsub(&mut self) { self.instr(InstrK::FSub) }
    fn i_fmul(&mut self) { self.instr(InstrK::FMul) }
    fn i_fdiv(&mut self) { self.instr(InstrK::FDiv) }
    fn i_itof(&mut self) { self.instr(InstrK::Itof) }
    fn i_ftoi(&mut self, int_ty: Ty<'ctx>) { self.instr(InstrK::Ftoi { int_ty }) }
    fn i_iconv(&mut self, target: Ty<'ctx>) { self.instr(InstrK::IConv { target }) }
    fn i_icmp(&mut self, cmp: Cmp) { self.instr(InstrK::ICmp(cmp)) }
    fn i_fcmp(&mut self, cmp: Cmp) { self.instr(InstrK::FCmp(cmp)) }
    fn i_not(&mut self) { self.instr(InstrK::Not) }
    fn i_bitand(&mut self) { self.instr(InstrK::BitAnd) }
    fn i_bitor(&mut self) { self.instr(InstrK::BitOr) }
    fn i_call(&mut self, func_name: String) { self.instr(InstrK::CallDirect { func_name }) }
    fn i_ld_local(&mut self, loc: LocalRef) { self.instr(InstrK::LdLocal { idx: loc.into() }) }
    fn i_st_local(&mut self, loc: LocalRef) { self.instr(InstrK::StLocal { idx: loc.into() }) }
    fn i_ld_global_func(&mut self, func_name: String) { self.instr(InstrK::LdGlobalFunc { func_name }) }
    fn i_call_indirect(&mut self) { self.instr(InstrK::CallIndirect) }
    fn i_bitcast(&mut self, target_type: Ty<'ctx>) { self.instr(InstrK::Bitcast { target: target_type }) }
    fn i_if_else(&mut self, then_block: BlockId, else_block: Option<BlockId>) {
        self.instr(InstrK::IfElse { then: then_block, r#else: else_block })
    }
    fn i_read(&mut self, ty: Ty<'ctx>) { self.instr(InstrK::Read { ty }) }
    fn i_write(&mut self, ty: Ty<'ctx>) { self.instr(InstrK::Write { ty }) }
    fn i_offset(&mut self, ty: Ty<'ctx>) { self.instr(InstrK::Offset { ty }) }
    fn i_get_field_ptr(&mut self, struct_ty: Ty<'ctx>, field_idx: usize) {
        self.instr(InstrK::GetFieldPtr { struct_ty, field_idx });
    }
    fn i_discard(&mut self) { self.instr(InstrK::Discard) }
    fn i_return(&mut self) { self.instr(InstrK::Return) }
    fn i_memory_size(&mut self) { self.instr(InstrK::MemorySize) }
    fn i_memory_grow(&mut self) { self.instr(InstrK::MemoryGrow) }
    fn i_ld_global(&mut self, name: String) { self.instr(InstrK::LdGlobal(name)) }
    fn i_st_global(&mut self, name: String) { self.instr(InstrK::StGlobal(name)) }
    fn i_fail(&mut self) { self.instr(InstrK::Fail) }
    fn i_loop(&mut self, body_block: BlockId) { self.instr(InstrK::Loop(body_block)) }
    fn i_break(&mut self) { self.instr(InstrK::Break) }
    fn i_ld_static_mem_ptr(&mut self, item: SMItemRef) { self.instr(InstrK::LdStaticMemPtr(item)) }
}

/// A wrapper which acts as a reference to a local.
#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct LocalRef(usize);

impl From<LocalRef> for usize {
    fn from(r: LocalRef) -> Self {
        r.0
    }
}