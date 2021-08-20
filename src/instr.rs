use std::{collections::HashMap, hint::unreachable_unchecked};

use crate::{metadata::Metadata, ty::{Ty, Type}};

pub enum InstrK<'ctx> {
    /// Load a constant integer value onto the stack
    LdInt(i32),
    /// Load a constant floating-point value onto the stack
    LdFloat(f32),
    /// Add two integers
    IAdd,
    /// Subtract two integers
    ISub,
    /// Multiply two integers
    IMul,
    /// Signed divide two integers. The result is undefined if the divisor is zero
    IDiv,
    /// Add two floating-point numbers.
    ///
    /// For precise semantics, see https://webassembly.github.io/spec/core/exec/numerics.html#op-fadd
    FAdd,
    /// Subtract two floating-point numbers.
    ///
    /// For precise semantics, see https://webassembly.github.io/spec/core/exec/numerics.html#op-fsub
    FSub,
    /// Multiply two floating-point numbers.
    ///
    /// For precise semantics, see https://webassembly.github.io/spec/core/exec/numerics.html#op-fmul
    FMul,
    /// Divide two floating-point numbers.
    ///
    /// For precise semantics, see https://webassembly.github.io/spec/core/exec/numerics.html#op-fdiv
    FDiv,
    /// Convert a signed integer to a floating-point number.
    ///
    /// Compiles to the `f32.convert_i32_s` instruction.
    Itof,
    /// Convert a floating-point number to a signed integer.
    ///
    /// Compiles to the `i64.trunc_sat_f32_s` instruction. 
    /// For precise semantics, see https://webassembly.github.io/spec/core/exec/numerics.html#op-trunc-sat-s
    Ftoi,
    /// Compare two signed integers. The result is an integer.
    ICmp(Cmp),
    /// Compare two floating-point values. The result is an integer.
    FCmp(Cmp),
    /// Call a global function by name.
    /// Pop arguments off the stack.
    CallDirect { func_name: String },
    /// Load the value of a local onto the stack
    LdLocal { idx: usize },
    /// Store the value on top of the stack into a local
    StLocal { idx: usize },
    /// Load a pointer to a global function onto the stack
    LdGlobalFunc { func_name: String },
    /// Call a function pointer on top of the stack.
    /// Pop arguments off the stack.
    CallIndirect,
    /// Signifies the end of a block, __every block must end with this instruction__.
    ///
    /// At the end of a block, execution either:
    /// * returns to the caller if this block is the main block of a function
    /// * returns to the parent block
    End,
    /// Cast a value to another type without any value conversions.
    /// equivalent to `*((T*)&expr)` in C.
    ///
    /// Fails to verify if the target and source types are of different sizes.
    Bitcast { target: Ty<'ctx> },
    /// Pop a value off the stack. If the value is non-zero, jump
    /// to the `then` block, otherwise jump to the `else` block (if there's one)
    IfElse { then: BlockId, r#else: Option<BlockId> },
    /// Pop a pointer off the stack and read a value of this type at the address of the pointer
    Read { ty: Ty<'ctx> },
    /// Pop a value and a pointer off the stack and write the value
    /// into memory at the address of the pointer
    Write { ty: Ty<'ctx> }
}

pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge
}

pub struct Instr<'ctx> {
    pub kind: InstrK<'ctx>,
    pub(crate) meta: Metadata
}

impl<'ctx> Instr<'ctx> {
    pub fn new(kind: InstrK<'ctx>) -> Self {
        Self { kind, meta: Metadata::new() }
    }
}

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug)]
pub struct BlockId(usize);

impl BlockId {
    #[inline]
    pub fn id(&self) -> usize {
        self.0
    }
}

impl From<usize> for BlockId {
    fn from(n: usize) -> Self { BlockId(n) }
}

impl From<BlockId> for usize {
    fn from(id: BlockId) -> Self { id.0 }
}

impl Default for BlockId {
    fn default() -> Self { BlockId(usize::MAX) }
}

pub struct InstrBlock<'ctx> {
    /// A unique index of the block inside a function.
    /// It's assigned by the builder and shouldn't be touched by the user
    pub(crate) idx: BlockId,
    pub body: Vec<Instr<'ctx>>,
    /// Every block has a type - it must be a function type with no arguments.
    ///
    /// The `return` of the function type describes what types are left on the stack
    /// once the block is exited.
    block_ty: Ty<'ctx>,
    pub(crate) meta: Metadata
}

impl<'ctx> InstrBlock<'ctx> {
    pub fn new(idx: BlockId, block_ty: Ty<'ctx>) -> Self {
        assert!(block_ty.is_func());
        if let Type::Func { args, ret: _ } = &*block_ty {
            assert!(args.is_empty());
        }

        InstrBlock { idx, body: Vec::new(), meta: Metadata::new(), block_ty }
    }
    /// A helper function to avoid doing `block.body.push(Instr::new(SMTH))`.
    /// Instead you can just do block.add(SMTH)
    pub fn add(&mut self, instr_k: InstrK<'ctx>) {
        self.body.push(Instr::new(instr_k))
    }

    pub fn full_type(&self) -> Ty<'ctx> {
        self.block_ty
    }

    pub fn returns(&self) -> &Vec<Ty<'ctx>> {
        match &*self.block_ty {
            Type::Func { args: _, ret } => ret,
            _ => unreachable!()
        }
    }

    #[inline]
    pub fn is_main(&self) -> bool {
        self.idx == BlockId(0)
    }
}

pub struct Function<'ctx> {
    name: String,
    ty: Ty<'ctx>,
    /// The entry block is always the first one (index zero)
    blocks: HashMap<BlockId, InstrBlock<'ctx>>,
    /// Types of the locals, including the arguments
    all_locals_types: Vec<Ty<'ctx>>,
    /// The function index inside the module. Should not be modified by anyone else than the module
    pub(crate) idx: usize
}

impl<'ctx> Function<'ctx> {
    pub(crate) fn new(
        name: String, 
        ty: Ty<'ctx>, 
        blocks: HashMap<BlockId, InstrBlock<'ctx>>,
        all_locals_types: Vec<Ty<'ctx>>
    /*additional_locals: impl IntoIterator<Item = Ty<'ctx>>*/) -> Self {
        
        assert!(ty.is_func(), "The type of a Function must be a function type");

        #[cfg(debug_assertions)]
        {
            let args_types = match &*ty {
                Type::Func { args, ret: _ } => args,
                _ => unsafe { unreachable_unchecked() }
            };

            debug_assert!(args_types == &all_locals_types[0..args_types.len()])
        }
        
        //let all_locals_types: Vec<_> = args_types.iter().copied().chain(additional_locals.into_iter()).collect();

        Function {
            name, ty, blocks, all_locals_types, idx: usize::MAX
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entry_block(&self) -> &InstrBlock<'ctx> {
        self.blocks.get(&0.into()).unwrap()
    }

    pub fn entry_block_mut(&mut self) -> &mut InstrBlock<'ctx> {
        self.blocks.get_mut(&0.into()).unwrap()
    }

    pub fn blocks_iter(&self) -> std::collections::hash_map::Values<'_, BlockId, InstrBlock<'ctx>> {
        self.blocks.values()
    }

    pub fn blocks_iter_mut(&mut self) -> std::collections::hash_map::ValuesMut<'_, BlockId, InstrBlock<'ctx>> {
        self.blocks.values_mut()
    }

    pub fn get_block(&self, id: BlockId) -> Option<&InstrBlock<'ctx>> {
        self.blocks.get(&id)
    }

    pub fn get_block_mut(&mut self, id: BlockId) -> Option<&mut InstrBlock<'ctx>> {
        self.blocks.get_mut(&id)
    }

    pub fn ret_tys(&self) -> &Vec<Ty<'ctx>> {
        match &*self.ty {
            crate::ty::Type::Func { args: _, ret } => ret,
            _ => unreachable!()
        }
    }

    pub fn arg_tys(&self) -> &Vec<Ty<'ctx>> {
        match &*self.ty {
            crate::ty::Type::Func { args, ret: _ } => args,
            _ => unreachable!()
        } 
    }

    pub fn ty(&self) -> Ty<'ctx> {
        self.ty
    }

    pub fn all_locals_ty(&self) -> &Vec<Ty<'ctx>> {
        &self.all_locals_types
    }

    pub fn local_ty(&self, idx: usize) -> Option<Ty<'ctx>> {
        self.all_locals_types.get(idx).copied()
    }

    pub fn all_local_count(&self) -> usize {
        self.all_locals_types.len()
    }

    pub fn arg_count(&self) -> usize {
        self.arg_tys().len()
    }

    pub fn ret_count(&self) -> usize {
        self.ret_tys().len()
    }
}