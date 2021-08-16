//! Function builder helps with creating valid IR.

use std::collections::HashMap;

use crate::{instr::{BlockId, Cmp, Function, Instr, InstrBlock, InstrK}, metadata::Metadata, module::Module, ty::{Ty, Type}};

pub struct FunctionBuilder<'ctx> {
    blocks: HashMap<BlockId, InstrBlock<'ctx>>,
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

        let mut entry_block = InstrBlock::new();
        entry_block.idx = 0.into();

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
            ret: returns.into_iter().collect(),
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

    pub fn new_block(&mut self) -> BlockId {
        let mut new_block = InstrBlock::new();
        let new_block_id = self.next_block_id.into();
        new_block.idx = new_block_id;
        self.blocks.insert(new_block_id, new_block);
        self.next_block_id += 1;
        new_block_id
    }

    pub fn switch_block(&mut self, new_current_block: BlockId) {
        assert!(self.blocks.contains_key(&new_current_block));
        self.current_block = new_current_block.into();
    }

    /// Finish building the current function and add it to the module
    pub fn finish(self, module: &mut Module<'ctx>) {
        let func_ty = module.intern_type(
            Type::Func { args: self.locals[0..self.argc].iter().copied().collect(), ret: self.ret }
        );
        let func = Function::new(
            self.fname,
            func_ty,
            self.blocks,
            self.locals
        );
        module.add_function(func);
    }
}

impl<'ctx> InstrBuilder for FunctionBuilder<'ctx> {
    fn instr(&mut self, i: InstrK) {
        let curr_block = self.current_block.into();
        self.blocks.get_mut(&curr_block).unwrap().body.push(
            Instr { kind: i, meta: Metadata::new() }
        );
    }
}

pub trait InstrBuilder {
    fn instr(&mut self, i: InstrK);

    fn i_ld_int(&mut self, val: i32) { self.instr(InstrK::LdInt(val)) }
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
    fn i_icmp(&mut self, cmp: Cmp) { self.instr(InstrK::ICmp(cmp)) }
    fn i_fcmp(&mut self, cmp: Cmp) { self.instr(InstrK::FCmp(cmp)) }
    fn i_call(&mut self, func_name: String) { self.instr(InstrK::CallDirect { func_name }) }
    fn i_ld_local(&mut self, loc: LocalRef) { self.instr(InstrK::LdLocal { idx: loc.into() }) }
    fn i_st_local(&mut self, loc: LocalRef) { self.instr(InstrK::StLocal { idx: loc.into() }) }
    fn i_ld_global_func(&mut self, func_name: String) { self.instr(InstrK::LdGlobalFunc { func_name }) }
    fn i_call_indirect(&mut self) { self.instr(InstrK::CallIndirect) }
    fn i_return(&mut self) { self.instr(InstrK::Return) }
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