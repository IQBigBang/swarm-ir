use std::{collections::HashMap, hint::unreachable_unchecked};

use crate::{intrinsic::{Intrinsic, Intrinsics}, metadata::{Key, Metadata}, ty::{Ty, Type}};

#[derive(PartialEq, Debug, Clone)]
pub enum InstrK<'ctx> {
    /// Load a constant integer value onto the stack
    LdInt(u32, Ty<'ctx>),
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
    /// For precise semantics, see <https://webassembly.github.io/spec/core/exec/numerics.html#op-fadd>
    FAdd,
    /// Subtract two floating-point numbers.
    ///
    /// For precise semantics, see <https://webassembly.github.io/spec/core/exec/numerics.html#op-fsub>
    FSub,
    /// Multiply two floating-point numbers.
    ///
    /// For precise semantics, see <https://webassembly.github.io/spec/core/exec/numerics.html#op-fmul>
    FMul,
    /// Divide two floating-point numbers.
    ///
    /// For precise semantics, see <https://webassembly.github.io/spec/core/exec/numerics.html#op-fdiv>
    FDiv,
    /// Convert a signed integer to a floating-point number.
    ///
    /// Compiles to the `f32.convert_i32_s` instruction.
    Itof,
    /// Convert a floating-point number to a signed integer.
    ///
    /// Compiles to the `i32.trunc_f32_s` or `i32.trunc_sat_f32_s` instruction
    /// depending on the IR Module configuration.
    /// For precise semantics, see <https://webassembly.github.io/spec/core/exec/numerics.html#op-trunc-sat-s>
    Ftoi { int_ty: Ty<'ctx> },
    /// Compare two signed integers. The result is an integer.
    ICmp(Cmp),
    /// Compare two floating-point values. The result is an integer.
    FCmp(Cmp),
    /// A boolean not operation on integers.
    /// Returns 1 if the value is 0 and 0 otherwise.
    Not,
    /// Bitwise and.
    BitAnd,
    /// Bitwise or.
    BitOr,
    /// Convert an integer to another integer type.
    ///
    /// For precise semantics, see the *numerics* draft, which contains a detailed description
    IConv { target: Ty<'ctx> },
    /// Call a global function by name.
    /// Pop arguments off the stack.
    CallDirect { func_name: String },
    /// Load the value of a local onto the stack
    LdLocal { idx: usize },
    /// Store the value on top of the stack into a local.
    /// 
    /// The local must not be an argument, as argument locals are immutable
    StLocal { idx: usize },
    /// Load a pointer to a global function onto the stack
    LdGlobalFunc { func_name: String },
    /// Call a function pointer on top of the stack.
    /// Pop arguments off the stack.
    CallIndirect,
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
    Write { ty: Ty<'ctx> },
    /// An operation to offset a pointer by an index as if it pointed to an array.
    /// Pops an integer `n` off the stack  and a pointer `ptr` and pushes a pointer
    /// whose address is equal to `(int)ptr + n * sizeof(T)`
    Offset { ty: Ty<'ctx> },
    /// Pop a pointer off the stack which points to `struct_ty`
    /// and push back a pointer which points to the Nth field of the struct
    GetFieldPtr { struct_ty: Ty<'ctx>, field_idx: usize },
    /// Pop a value off the stack and discard it
    Discard,
    /// Return immediately from the current function.
    ///
    /// The stack must contain _exactly_ the number of values the function returns.
    ///
    /// This instruction terminates a block, it shouldn't be followed by any more instructions.
    Return,
    /// Corresponds to the WebAssembly [`memory.size`](https://webassembly.github.io/spec/core/exec/instructions.html#xref-syntax-instructions-syntax-instr-memory-mathsf-memory-size) instruction.
    ///
    /// Pushes an integer.
    MemorySize,
    /// Corresponds to the WebAssembly [`memory.grow`](https://webassembly.github.io/spec/core/exec/instructions.html#xref-syntax-instructions-syntax-instr-memory-mathsf-memory-grow) instruction.
    ///
    /// Pops an integer off the stack and pushes a new one.
    MemoryGrow,
    /// Load the value of a global and push it onto the stack
    LdGlobal(String),
    /// Pop a value off the stack and store it into a global
    StGlobal(String),
    /// Fail.
    ///
    /// This instruction is always assumed to produce stack values
    /// valid for the current context.
    ///
    /// On WebAssembly, this instruction traps.
    ///
    /// All instructions in a block following this one
    /// are considered unreachable and will be removed.
    Fail,
    /// Repeatedly execute a block of code.
    /// 
    /// The block must have a void type ([] -> [])
    Loop(BlockId),
    /// Break from the innermost loop.
    Break,
    /// An intrinsic is a private instruction used for analysis, optimization etc.
    Intrinsic(Intrinsic<'ctx>)
}

#[repr(C)]
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge
}

#[derive(Clone)]
pub struct Instr<'ctx> {
    pub kind: InstrK<'ctx>,
    pub(crate) meta: Metadata<'ctx, Key>
}

impl<'ctx> Instr<'ctx> {
    pub fn new(kind: InstrK<'ctx>) -> Self {
        Self { kind, meta: Metadata::new() }
    }

    pub(crate) fn new_with_meta(kind: InstrK<'ctx>, meta: Metadata<'ctx, Key>) -> Self {
        Self { kind, meta }
    }

    pub(crate) fn new_intrinsic(i: Intrinsics<'ctx>) -> Self {
        Self { kind: InstrK::Intrinsic(Intrinsic(i)), meta: Metadata::new() }
    }

    /// Return true if this instruction is a "load" instruction.
    /// A "load" instruction is an instruction which pops no values off the stack and pushes exactly one value.
    ///
    /// Namely this includes LdInt, LdFloat, LdLocal, LdGlobalFunc
    pub fn is_load(&self) -> bool {
        matches!(self.kind, InstrK::LdInt(_, _) | InstrK::LdFloat(_) | InstrK::LdLocal { idx: _ } | InstrK::LdGlobalFunc { func_name: _ })
    }

    /// Return true if this instruction is a "diverging" instruction.
    /// 
    /// Namely this includes Return, Fail and Break
    pub fn is_diverging(&self) -> bool {
        matches!(self.kind, InstrK::Return | InstrK::Fail | InstrK::Break)
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

    pub fn entry_block_id() -> Self { Self(0) }
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

/// Defines how the block is used
#[repr(C)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum BlockTag {
    Undefined,
    /// The "main" block of the function
    Main,
    /// A block which is used as one of the branches of an IfElse instruction
    IfElse,
    /// A block which is used as the body of a Loop instruction
    Loop,
}

/// A block is a series of instructions
/// which are guaranteed to be executed in order they're defined,
/// i.e. for a block consisting of instructions `A, B, C`, `C` is
/// guaranteed to be executed before `B` and `B` is guaranteed to be executed before `A`.
/// 
/// At the end of a block's execution, the behavior depends on the tag:
/// * If tag = *main*, then the function returns
/// * If tag = *ifelse*, then the execution jumps to the instruction one after the corresponding `if_else` instruction
/// * If tag = *loop*, then the execution jumps to the start of the loop block
/// Otherwise, the behavior is not specified.
pub struct InstrBlock<'ctx> {
    /// A unique index of the block inside a function.
    /// It's assigned by the builder and shouldn't be touched by the user
    pub(crate) idx: BlockId,
    pub(crate) tag: BlockTag,
    pub body: Vec<Instr<'ctx>>,
    /// Every block has a type - it must be a function type with no arguments.
    ///
    /// The `return` of the function type describes what types are left on the stack
    /// once the block is exited.
    block_ty: Ty<'ctx>,
    pub(crate) meta: Metadata<'ctx, Key>
}

impl<'ctx> InstrBlock<'ctx> {
    pub fn new(idx: BlockId, block_ty: Ty<'ctx>, tag: BlockTag) -> Self {
        assert!(block_ty.is_func());
        if let Type::Func { args, ret: _ } = &*block_ty {
            assert!(args.is_empty());
        }

        InstrBlock { idx, tag, body: Vec::new(), meta: Metadata::new(), block_ty }
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

    #[inline]
    pub fn tag(&self) -> BlockTag { self.tag }
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

    /// Returns true if the Nth local is actually an argument to the function
    pub fn is_local_an_arg(&self, n: usize) -> bool {
        n < self.arg_count()
    }
}