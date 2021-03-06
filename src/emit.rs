use std::{collections::HashMap, convert::TryInto, marker::PhantomData};

use wasm_encoder as wasm;

use crate::{abi::Abi, instr::{Cmp, Function, InstrBlock, InstrK}, module::{FuncDef, Functional, Module}, numerics::{emit_numeric_instr, type_to_bws}, pass::FunctionPass, staticmem::{CompiledStaticMemory, SMItemRef}, ty::{Ty, Type}};

pub struct WasmEmitter<'ctx, A: Abi> {
    module: wasm::Module,
    /// A table of function types and their indexes in the resulting wasm module
    function_types: HashMap<Ty<'ctx>, u32>,
    /// Memory addresses of items in static memory
    static_memory_addresses: HashMap<SMItemRef, usize>,

    /* Follow the sections. Because the Wasm specification requires a certain order,
    the sections are saved separately and only combined into the module file at the very end */
    /// Defines mainly the function types
    type_sec: wasm::TypeSection,
    /// Defines what external items are imported
    import_sec: wasm::ImportSection, 
    /// Defines the functions (function prototypes)
    func_sec: wasm::FunctionSection,
    /// Defines the tables, right now there's only one table: the global function table
    table_sec: wasm::TableSection,
    /// Defines the memory
    memory_sec: wasm::MemorySection,
    /// Defines the global items
    global_sec: wasm::GlobalSection,
    /// Defines what items (functions, memories) are exported
    export_sec: wasm::ExportSection,
    /// Defines the elements of the global function table
    elem_sec: wasm::ElementSection,
    /// Defines the actual code of the functions
    code_sec: wasm::CodeSection,
    /// Defines the data segments which initialize memory
    data_sec: wasm::DataSection,
    /// Defines the debug names of symbols.
    // TODO: support this properly
    name_sec: wasm::NameSection,
    _ph: PhantomData<A>
}

// The Abi must be wasm-compatible, therefore the type specification
impl<'ctx, A: Abi<BackendType = wasm::ValType>> WasmEmitter<'ctx, A> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        WasmEmitter {
            module: wasm::Module::new(),
            function_types: HashMap::new(),
            static_memory_addresses: HashMap::new(),

            type_sec: wasm::TypeSection::new(),
            import_sec: wasm::ImportSection::new(),
            func_sec: wasm::FunctionSection::new(),
            table_sec: wasm::TableSection::new(),
            memory_sec: wasm::MemorySection::new(),
            global_sec: wasm::GlobalSection::new(),
            export_sec: wasm::ExportSection::new(),
            elem_sec: wasm::ElementSection::new(),
            code_sec: wasm::CodeSection::new(),
            data_sec: wasm::DataSection::new(),
            name_sec: wasm::NameSection::new(),
            _ph: PhantomData
        }
    }

    fn encode_types(&mut self, module: &Module<'ctx>) {
        module.for_all_types_iter(|ty| {
            if let Type::Func { args, ret } = &*ty {
                self.type_sec.function(
                    args.iter().map(|t| A::compile_type(*t)),
                    ret.iter().map(|t| A::compile_type(*t)) 
                );
                // The function type is the last one
                let idx = self.type_sec.len() - 1;
                self.function_types.insert(ty, idx);
            }
        })
    }

    fn compile_func(&mut self, module: &Module<'ctx>, func: &Function<'ctx>) {
        // First actually compile the function
        // the locals passed to wasm::Function are only additional locals, WITHOUT the arguments

        let mut out_f = wasm::Function::new_with_locals_types(
            func.all_locals_ty().iter()
                .skip(func.arg_count())
                .map(|t| A::compile_type(*t)));

        self.compile_block(
            module, 
            func, 
            func.entry_block(), 
            &mut out_f);
        out_f.instruction(&wasm::Instruction::End);

        // Then add to the sections
        // first the function section
        self.func_sec.function(self.function_types[&func.ty()]);
        // then the export section
        // TODO: specify whether the function should be exported
        self.export_sec.export(func.name(), wasm::Export::Function(func.idx as u32));
        // then the code section
        self.code_sec.function(&out_f);
    }

    fn compile_block(
        &mut self, 
        module: &Module<'ctx>, 
        function: &Function<'ctx>, 
        block: &InstrBlock<'ctx>, 
        out_f: &mut wasm::Function) {
        for instr in &block.body {
            match &instr.kind {
                InstrK::LdInt(val, _) => { out_f.instruction(&wasm::Instruction::I32Const(*val as i32)); },
                InstrK::LdFloat(val) => { out_f.instruction(&wasm::Instruction::F32Const(*val)); },
                InstrK::FAdd => { out_f.instruction(&wasm::Instruction::F32Add); },
                InstrK::FSub => { out_f.instruction(&wasm::Instruction::F32Sub); },
                InstrK::FMul => { out_f.instruction(&wasm::Instruction::F32Mul); },
                InstrK::FDiv => { out_f.instruction(&wasm::Instruction::F32Div); },
                // these are all numerics WITH metadata
                InstrK::IAdd | InstrK::ISub | InstrK::IMul | InstrK::IDiv |
                InstrK::Itof | InstrK::ICmp(_) | InstrK::IConv { target: _ } => {
                    let bws = instr.meta.retrieve_copied(key!("bws")).unwrap();
                    let instrs = emit_numeric_instr::<A>(&instr.kind, bws, module.conf.use_saturating_ftoi);
                    for i in instrs { out_f.instruction(&i); }
                },
                InstrK::Ftoi { int_ty } => { 
                    // numeric without metadata, calculate bws from the explicit type
                    let bws = type_to_bws(*int_ty).unwrap();
                    let instrs = emit_numeric_instr::<A>(&instr.kind, bws, module.conf.use_saturating_ftoi);
                    for i in instrs { out_f.instruction(&i); }
                },
                InstrK::FCmp(cmp) => { match cmp {
                    Cmp::Eq => out_f.instruction(&wasm::Instruction::F32Eq),
                    Cmp::Ne => out_f.instruction(&wasm::Instruction::F32Neq),
                    Cmp::Lt => out_f.instruction(&wasm::Instruction::F32Lt),
                    Cmp::Le => out_f.instruction(&wasm::Instruction::F32Le),
                    Cmp::Gt => out_f.instruction(&wasm::Instruction::F32Gt),
                    Cmp::Ge => out_f.instruction(&wasm::Instruction::F32Ge),
                }; }
                InstrK::Not => {
                    // !x is the same as (x == 0)
                    out_f.instruction(&wasm::Instruction::I32Eqz);
                }
                InstrK::BitAnd => {
                    out_f.instruction(&wasm::Instruction::I32And);
                }
                InstrK::BitOr => {
                    out_f.instruction(&wasm::Instruction::I32Or);
                }
                InstrK::CallDirect { func_name } => {
                    let func_idx = module.get_function(func_name).unwrap().idx();
                    out_f.instruction(&wasm::Instruction::Call(func_idx.try_into().unwrap()));
                },
                InstrK::LdLocal { idx } => { out_f.instruction(&wasm::Instruction::LocalGet(*idx as u32)); },
                InstrK::StLocal { idx } => { out_f.instruction(&wasm::Instruction::LocalSet(*idx as u32)); },
                InstrK::LdGlobalFunc { func_name } => {
                    let func_idx = module.get_function(func_name).unwrap().idx();
                    // the index must be shifted by one - see the description of [`emit_global_function_table`]
                    out_f.instruction(&wasm::Instruction::I32Const((func_idx + 1).try_into().unwrap()));
                },
                InstrK::CallIndirect => {
                    // meta["ty"] injected by the Verifier
                    let function_ty = instr.meta.retrieve_ty(key!("ty")).unwrap();
                    out_f.instruction(&wasm::Instruction::CallIndirect {
                        ty: self.function_types[&function_ty],
                        table: 0 // the GFT is the only one and it's at index zero
                    });
                },
                InstrK::Bitcast { target } => {
                    // meta["from"] injected by the Verifier
                    let from = instr.meta.retrieve_ty(key!("from")).unwrap();
                    let from_wasm = A::compile_type(from);
                    let to_wasm = A::compile_type(*target);
                    match (from_wasm, to_wasm) {
                        (wasm::ValType::I32, wasm::ValType::I32) | (wasm::ValType::F32, wasm::ValType::F32) => { /* no-op */ }
                        (wasm::ValType::I32, wasm::ValType::F32) => {
                            out_f.instruction(&wasm::Instruction::F32ReinterpretI32);
                        }
                        (wasm::ValType::F32, wasm::ValType::I32) => {
                            out_f.instruction(&wasm::Instruction::I32ReinterpretF32);
                        }
                        /* We don't currently use other types (function "pointers" are implemented as I32) */
                        _ => unimplemented!()
                    }
                }
                InstrK::IfElse { then, r#else } => {
                    let block = function.get_block(*then).unwrap();
                    let block_type = 
                        if block.returns().is_empty() {
                            wasm::BlockType::Empty
                        } else if block.returns().len() == 1 {
                            // If the block returns only a single value, prefer
                            // to compile it as returning that one value
                            // rather than a function type
                            wasm::BlockType::Result(A::compile_type(block.returns()[0]))
                        } else {
                            wasm::BlockType::FunctionType(self.function_types[&block.full_type()])
                        };
                    out_f.instruction(&wasm::Instruction::If(block_type));
                    // compile the `then` block
                    // according to wasm spec, it doesn't need the end instruction
                    self.compile_block(module, function, block, out_f);
                    
                    out_f.instruction(&wasm::Instruction::Else);
                    match r#else {
                        Some(idx) =>
                            self.compile_block(module, function, function.get_block(*idx).unwrap(), out_f),
                        None => {}
                    }
                    out_f.instruction(&wasm::Instruction::End);
                }
                InstrK::Read { ty } => {
                    if ty.is_int() {
                        // use the "numeric" module functions for compilation
                        let bws = type_to_bws(*ty).unwrap();
                        let instrs = emit_numeric_instr::<A>(&instr.kind, bws, module.conf.use_saturating_ftoi);
                        for i in instrs { out_f.instruction(&i); }
                        continue
                    }

                    let mem_arg = wasm::MemArg {
                        offset: 0,
                        align: A::type_alignment(*ty) as u32,
                        memory_index: 0,
                    };

                    match A::compile_type(*ty) {
                        wasm::ValType::F32 => {
                            out_f.instruction(&wasm::Instruction::F32Load(mem_arg));
                        },
                        _ => unimplemented!()
                    }
                }
                InstrK::Write { ty } => {
                    if ty.is_int() {
                        // use the "numeric" module functions for compilation
                        let bws = type_to_bws(*ty).unwrap();
                        let instrs = emit_numeric_instr::<A>(&instr.kind, bws, module.conf.use_saturating_ftoi);
                        for i in instrs { out_f.instruction(&i); }
                        continue
                    }

                    let mem_arg = wasm::MemArg {
                        offset: 0,
                        align: A::type_alignment(*ty) as u32,
                        memory_index: 0,
                    };

                    match A::compile_type(*ty) {
                        wasm::ValType::F32 => {
                            out_f.instruction(&wasm::Instruction::F32Store(mem_arg));
                        },
                        _ => unimplemented!()
                    }
                }
                InstrK::Offset { ty } => {
                    // we need to calculate stack(0) * sizeof(ty) + stack(1)
                    // the sequence we'll use is:
                    // LdInt sizeof(ty)
                    // IMul
                    // IAdd
                    // but because the sizes are often powers of two, for optimization
                    // purposes we'll replace the multiplications with left-shifts:
                    match A::type_sizeof(*ty) {
                        1 => {}, // no multiplication
                        2 => {
                            out_f.instruction(&wasm::Instruction::I32Const(1));
                            out_f.instruction(&wasm::Instruction::I32Shl);
                        }
                        4 => {
                            out_f.instruction(&wasm::Instruction::I32Const(2));
                            out_f.instruction(&wasm::Instruction::I32Shl);
                        }
                        8 => {
                            out_f.instruction(&wasm::Instruction::I32Const(3));
                            out_f.instruction(&wasm::Instruction::I32Shl);
                        }
                        other => {
                            out_f.instruction(&wasm::Instruction::I32Const(other as i32));
                            out_f.instruction(&wasm::Instruction::I32Mul);
                        }
                    }
                    // finally the `IAdd`
                    out_f.instruction(&wasm::Instruction::I32Add);
                }
                InstrK::GetFieldPtr { struct_ty, field_idx } => {
                    // The `GetFieldPtr` instruction is basically
                    // just an addition with a correct offset
                    // Calculate the offset
                    let struct_fields = match &**struct_ty {
                        Type::Struct { fields } => fields,
                        _ => unreachable!()
                    };
                    let field_offset = A::struct_field_offset(struct_fields, *field_idx);
                    // emit the addition
                    // opt: if the field_offset is zero, we don't need to emit I32Const(0) followed by IAdd
                    if field_offset != 0 {
                        out_f.instruction(&wasm::Instruction::I32Const(field_offset as i32));
                        out_f.instruction(&wasm::Instruction::I32Add);
                    }
                },
                InstrK::Discard => { out_f.instruction(&wasm::Instruction::Drop); }
                InstrK::Return => { 
                    out_f.instruction(&wasm::Instruction::Return);
                }
                InstrK::MemorySize => { out_f.instruction(&wasm::Instruction::MemorySize(0)); }
                InstrK::MemoryGrow => { out_f.instruction(&wasm::Instruction::MemoryGrow(0)); }
                InstrK::LdGlobal(name) => {
                    out_f.instruction(&wasm::Instruction::GlobalGet(module.get_global(name).unwrap().idx() as u32));
                }
                InstrK::StGlobal(name) => {
                    out_f.instruction(&wasm::Instruction::GlobalSet(module.get_global(name).unwrap().idx() as u32));
                }
                InstrK::Fail => { out_f.instruction(&wasm::Instruction::Unreachable); }
                InstrK::Loop(body) => {
                    // The loop body's type is always () -> ()
                    debug_assert!(function.get_block(*body).unwrap().returns().is_empty());
                    let body_block_type = wasm::BlockType::Empty;
                    // We emit (block (loop <body> br 0))
                    out_f.instruction(&wasm::Instruction::Block(body_block_type));
                    out_f.instruction(&wasm::Instruction::Loop(body_block_type));
                    self.compile_block(module, function, function.get_block(*body).unwrap(), out_f);
                    // This `br 0` is what ensures the looping
                    out_f.instruction(&wasm::Instruction::Br(0));
                    out_f.instruction(&wasm::Instruction::End);
                    out_f.instruction(&wasm::Instruction::End);
                }
                InstrK::Break => {
                    // Per the ControlFlow2 proposal, `Break` compiles to
                    // br(innermost_loop_distance_of_this_block + 1)
                    let ilp: usize = block.meta.retrieve_copied(key!("innermost_loop_distance")).unwrap();
                    out_f.instruction(&wasm::Instruction::Br((ilp + 1) as u32));
                }
                InstrK::LdStaticMemPtr(item) => {
                    out_f.instruction(&wasm::Instruction::I32Const(
                        self.static_memory_addresses[item] as i32));
                }
                InstrK::Intrinsic(_i) => {
                    // TODO: alter the ReadAtOffset and WriteAtOffset instruction to work with other integral types
                    unimplemented!()
                }
            };
        }
    } 

    fn emit_memory_section(&mut self, initial_memory_size: u32) {
        self.memory_sec.memory(wasm::MemoryType {
            minimum: initial_memory_size as u64,
            maximum: None, // TODO
            memory64: false,
        });
    }

    /// The global function table is a table which contains funcrefs
    /// to all the defined functions. It's required so that function pointers (read "passing functions as values")
    /// is possible.
    /// The indexes into the GFT are off by one in comparison to function indexes, so that
    /// the function "pointer" with value zero is not a valid one. (preserves common semantics of pointers)
    fn emit_global_function_table(&mut self, module: &Module<'ctx>) {
        let table_length: u32 =
            TryInto::<u32>::try_into(module.function_count()).unwrap() 
            + 1; // +1 to account for the shift by one
        
        self.table_sec.table(wasm::TableType {
            element_type: wasm::ValType::FuncRef,
            minimum: table_length, 
            maximum: Some(table_length),
        });

        let functions_indexes: Vec<_> = 
            (0u32..(module.function_count() as u32)).collect();

        // An active element section initializes the table at start
        self.elem_sec.active(
            Some(0), 
            &wasm::Instruction::I32Const(1), // skip the first element 
            wasm::ValType::FuncRef, 
            wasm::Elements::Functions(&functions_indexes));
    }

    fn emit_globals(&mut self, module: &Module<'ctx>) {
        for glob in module.globals_iter() {
            let init_expr = if glob.is_int() {
                wasm::Instruction::I32Const(glob.get_int_value())
            } else {
                wasm::Instruction::F32Const(glob.get_float_value())
            };

            self.global_sec.global(
                wasm::GlobalType {
                    val_type: A::compile_type(glob.ty),
                    mutable: true // TODO immutable globals
                }, &init_expr);
        }
    }

    /// Emit `extern` definitions = imports in WASM
    fn emit_externs(&mut self, module: &Module<'ctx>) {
        for f in module.functions_iter() {
            let f = match f {
                FuncDef::Extern(x) => x,
                _ => break
            };
            
            // TODO: configure custom import module name
            // we default to `env` because that's what LLVM (= C++ and Rust) do
            self.import_sec.import(
                "env", 
                Some(f.name()), 
                wasm::EntityType::Function(self.function_types[&f.ty()])
            );
        }
    }

    pub fn compile_static_memory(&mut self, module: &Module<'ctx>) {
        if let Some(mem) = module.get_static_memory() {
            let compiled_mem = CompiledStaticMemory::compile::<A>(module, mem);
            self.data_sec.active(
                0, 
                &wasm::Instruction::I32Const(0), // no offset
                compiled_mem.buf);
            // Assign the addresses
            self.static_memory_addresses = compiled_mem.addresses;
        }
    }

    pub fn finish(mut self) -> Vec<u8> {
        // Emit the sections in correct order
        self.module
            .section(&self.type_sec)
            .section(&self.import_sec)
            .section(&self.func_sec)
            .section(&self.table_sec)
            .section(&self.memory_sec)
            .section(&self.global_sec)
            .section(&self.export_sec)
            .section(&self.elem_sec)
            .section(&self.code_sec)
            .section(&self.data_sec)
            .section(&self.name_sec);
        self.module.finish()
    }
}

impl<'ctx, A: Abi<BackendType = wasm::ValType>> FunctionPass<'ctx> for WasmEmitter<'ctx, A> {
    type Error = (); // TODO some error
    type Output = ();

    fn visit_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> {
        // this must be done before visiting the functions
        self.encode_types(module);
        // emit globals' definitions
        self.emit_globals(module);
        // compile the static memory - must happen after globals
        self.compile_static_memory(module);
        // emit external (i.e. imported) definitions
        self.emit_externs(module);
        Ok(())
    }

    fn visit_function(
        &mut self, 
        module: &Module<'ctx>,
        function: &Function<'ctx>) -> Result<(), Self::Error> {
        
        self.compile_func(module, function);
        Ok(())
    }

    fn end_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> {
        self.emit_memory_section(module.conf.initial_memory_size);
        self.emit_global_function_table(module);
        Ok(())
    }
}