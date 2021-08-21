use std::{collections::HashMap, convert::TryInto, marker::PhantomData};

use wasm_encoder as wasm;

use crate::{abi::Abi, instr::{Cmp, Function, InstrBlock, InstrK}, module::Module, pass::FunctionPass, ty::{Ty, Type}};

pub struct WasmEmitter<'ctx, A: Abi> {
    module: wasm::Module,
    /// A table of function types and their indexes in the resulting wasm module
    function_types: HashMap<Ty<'ctx>, u32>,

    /* Follow the sections. Because the Wasm specification requires a certain order,
    the sections are saved separately and only combined into the module file at the very end */
    /// Defines mainly the function types
    type_sec: wasm::TypeSection,
    /// Defines the functions (function prototypes)
    func_sec: wasm::FunctionSection,
    /// Defines the tables, right now there's only one table: the global function table
    table_sec: wasm::TableSection,
    /// Defines the memory
    memory_sec: wasm::MemorySection,
    /// Defines what items (functions, memories) are exported
    export_sec: wasm::ExportSection,
    /// Defines the elements of the global function table
    elem_sec: wasm::ElementSection,
    /// Defines the actual code of the functions
    code_sec: wasm::CodeSection,
    _ph: PhantomData<A>
}

// The Abi must be wasm-compatible, therefore the type specification
impl<'ctx, A: Abi<BackendType = wasm::ValType>> WasmEmitter<'ctx, A> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        WasmEmitter {
            module: wasm::Module::new(),
            function_types: HashMap::new(),

            type_sec: wasm::TypeSection::new(),
            func_sec: wasm::FunctionSection::new(),
            table_sec: wasm::TableSection::new(),
            memory_sec: wasm::MemorySection::new(),
            export_sec: wasm::ExportSection::new(),
            elem_sec: wasm::ElementSection::new(),
            code_sec: wasm::CodeSection::new(),
            _ph: PhantomData
        }
    }

    fn encode_types(&mut self, module: &Module<'ctx>) {
        for ty in module.all_types_iter() {
            if let Type::Func { args, ret } = &*ty {
                self.type_sec.function(
                    args.iter().map(|t| A::compile_type(*t)),
                    ret.iter().map(|t| A::compile_type(*t)) 
                );
                // The function type is the last one
                let idx = self.type_sec.len() - 1;
                self.function_types.insert(ty, idx);
            }
        }
    }

    fn compile_func(&mut self, module: &Module<'ctx>, func: &Function<'ctx>) {
        // First actually compile the function
        // the locals passed to wasm::Function are only additional locals, WITHOUT the arguments
        let local_iter = 
            (func.arg_count() as u32 .. func.all_local_count() as u32)
            .zip(
                func.all_locals_ty().iter()
                .skip(func.arg_count())
                .map(|t| A::compile_type(*t)));
        
        let mut out_f = wasm::Function::new(local_iter);

        self.compile_block(
            module, 
            func, 
            func.entry_block(), 
            &mut out_f,
        true);

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
        out_f: &mut wasm::Function,
        // Some control flow constructs require the `end` to be emitted manually and/or in a different place
        emit_end: bool) {
        for instr in &block.body {
            match &instr.kind {
                InstrK::LdInt(val) => { out_f.instruction(wasm::Instruction::I32Const(*val)); },
                InstrK::LdFloat(val) => { out_f.instruction(wasm::Instruction::F32Const(*val)); },
                InstrK::IAdd => { out_f.instruction(wasm::Instruction::I32Add); },
                InstrK::ISub => { out_f.instruction(wasm::Instruction::I32Sub); },
                InstrK::IMul => { out_f.instruction(wasm::Instruction::I32Mul); },
                InstrK::IDiv => { out_f.instruction(wasm::Instruction::I32DivS); },
                InstrK::FAdd => { out_f.instruction(wasm::Instruction::F32Add); },
                InstrK::FSub => { out_f.instruction(wasm::Instruction::F32Sub); },
                InstrK::FMul => { out_f.instruction(wasm::Instruction::F32Mul); },
                InstrK::FDiv => { out_f.instruction(wasm::Instruction::F32Div); },
                InstrK::Itof => { out_f.instruction(wasm::Instruction::F32ConvertI32S); },
                InstrK::Ftoi => { out_f.instruction(wasm::Instruction::I32TruncSatF32S); },
                InstrK::ICmp(cmp) => { match cmp {
                    Cmp::Eq => out_f.instruction(wasm::Instruction::I32Eq),
                    Cmp::Ne => out_f.instruction(wasm::Instruction::I32Neq),
                    Cmp::Lt => out_f.instruction(wasm::Instruction::I32LtS),
                    Cmp::Le => out_f.instruction(wasm::Instruction::I32LeS),
                    Cmp::Gt => out_f.instruction(wasm::Instruction::I32GtS),
                    Cmp::Ge => out_f.instruction(wasm::Instruction::I32GeS),
                }; }
                InstrK::FCmp(cmp) => { match cmp {
                    Cmp::Eq => out_f.instruction(wasm::Instruction::F32Eq),
                    Cmp::Ne => out_f.instruction(wasm::Instruction::F32Neq),
                    Cmp::Lt => out_f.instruction(wasm::Instruction::F32Lt),
                    Cmp::Le => out_f.instruction(wasm::Instruction::F32Le),
                    Cmp::Gt => out_f.instruction(wasm::Instruction::F32Gt),
                    Cmp::Ge => out_f.instruction(wasm::Instruction::F32Ge),
                }; }
                InstrK::CallDirect { func_name } => {
                    let func_idx = module.get_function(func_name).unwrap().idx;
                    out_f.instruction(wasm::Instruction::Call(func_idx.try_into().unwrap()));
                },
                InstrK::LdLocal { idx } => { out_f.instruction(wasm::Instruction::LocalGet(*idx as u32)); },
                InstrK::StLocal { idx } => { out_f.instruction(wasm::Instruction::LocalSet(*idx as u32)); },
                InstrK::LdGlobalFunc { func_name } => {
                    let func_idx = module.get_function(func_name).unwrap().idx;
                    // the index must be shifted by one - see the description of [`emit_global_function_table`]
                    out_f.instruction(wasm::Instruction::I32Const((func_idx + 1).try_into().unwrap()));
                },
                InstrK::CallIndirect => {
                    // meta["ty"] injected by the Verifier
                    let function_ty = instr.meta.retrieve_ty("ty").unwrap();
                    out_f.instruction(wasm::Instruction::CallIndirect {
                        ty: self.function_types[&function_ty],
                        table: 0 // the GFT is the only one and it's at index zero
                    });
                },
                InstrK::Bitcast { target } => {
                    // meta["from"] injected by the Verifier
                    let from = instr.meta.retrieve_ty("from").unwrap();
                    let from_wasm = A::compile_type(from);
                    let to_wasm = A::compile_type(*target);
                    match (from_wasm, to_wasm) {
                        (wasm::ValType::I32, wasm::ValType::I32) | (wasm::ValType::F32, wasm::ValType::F32) => { /* no-op */ }
                        (wasm::ValType::I32, wasm::ValType::F32) => {
                            out_f.instruction(wasm::Instruction::F32ReinterpretI32);
                        }
                        (wasm::ValType::F32, wasm::ValType::I32) => {
                            out_f.instruction(wasm::Instruction::I32ReinterpretF32);
                        }
                        /* We don't currently use other types (function "pointers" are implemented as I32) */
                        _ => unimplemented!()
                    }
                }
                InstrK::End => { if emit_end { out_f.instruction(wasm::Instruction::End); } },
                InstrK::IfElse { then, r#else } => {
                    let then_block_type = function.get_block(*then).unwrap().full_type();
                    let block_type = wasm::BlockType::FunctionType(self.function_types[&then_block_type]);
                    out_f.instruction(wasm::Instruction::If(block_type));
                    // compile the `then` block
                    ///// the block already ends with `end`, we don't need to add it
                    self.compile_block(module, function, function.get_block(*then).unwrap(), out_f, false);
                    
                    out_f.instruction(wasm::Instruction::Else);
                    match r#else {
                        Some(idx) =>
                            self.compile_block(module, function, function.get_block(*idx).unwrap(), out_f, true),
                        None => {
                            out_f.instruction(wasm::Instruction::End);
                        }
                    }
                }
                InstrK::Read { ty } => {
                    let mem_arg = wasm::MemArg {
                        offset: 0,
                        align: A::type_alignment(*ty) as u32,
                        memory_index: 0,
                    };

                    match A::compile_type(*ty) {
                        wasm::ValType::I32 => {
                            out_f.instruction(wasm::Instruction::I32Load(mem_arg));
                        },
                        wasm::ValType::F32 => {
                            out_f.instruction(wasm::Instruction::F32Load(mem_arg));
                        },
                        _ => unimplemented!()
                    }
                }
                InstrK::Write { ty } => {
                    let mem_arg = wasm::MemArg {
                        offset: 0,
                        align: A::type_alignment(*ty) as u32,
                        memory_index: 0,
                    };

                    match A::compile_type(*ty) {
                        wasm::ValType::I32 => {
                            out_f.instruction(wasm::Instruction::I32Store(mem_arg));
                        },
                        wasm::ValType::F32 => {
                            out_f.instruction(wasm::Instruction::F32Store(mem_arg));
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
                            out_f.instruction(wasm::Instruction::I32Const(1));
                            out_f.instruction(wasm::Instruction::I32Shl);
                        }
                        4 => {
                            out_f.instruction(wasm::Instruction::I32Const(2));
                            out_f.instruction(wasm::Instruction::I32Shl);
                        }
                        8 => {
                            out_f.instruction(wasm::Instruction::I32Const(3));
                            out_f.instruction(wasm::Instruction::I32Shl);
                        }
                        other => {
                            out_f.instruction(wasm::Instruction::I32Const(other as i32));
                            out_f.instruction(wasm::Instruction::I32Mul);
                        }
                    }
                    // finally the `IAdd`
                    out_f.instruction(wasm::Instruction::I32Add);
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
                    out_f.instruction(wasm::Instruction::I32Const(field_offset as i32));
                    out_f.instruction(wasm::Instruction::I32Add);
                },
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
            TryInto::<u32>::try_into(module.functions_iter().len()).unwrap() 
            + 1; // +1 to account for the shift by one
        
        self.table_sec.table(wasm::TableType {
            element_type: wasm::ValType::FuncRef,
            minimum: table_length, 
            maximum: Some(table_length),
        });

        let functions_indexes: Vec<_> = 
            (0u32..(module.functions_iter().len() as u32)).collect();

        // An active element section initializes the table at start
        self.elem_sec.active(
            Some(0), 
            wasm::Instruction::I32Const(1), // skip the first element 
            wasm::ValType::FuncRef, 
            wasm::Elements::Functions(&functions_indexes));
    }

    pub fn finish(mut self) -> Vec<u8> {
        // Emit the sections in correct order
        self.module
            .section(&self.type_sec)
            .section(&self.func_sec)
            .section(&self.table_sec)
            .section(&self.memory_sec)
            .section(&self.export_sec)
            .section(&self.elem_sec)
            .section(&self.code_sec);
        self.module.finish()
    }
}

impl<'ctx, A: Abi<BackendType = wasm::ValType>> FunctionPass<'ctx> for WasmEmitter<'ctx, A> {
    type Error = (); // TODO some error

    fn visit_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> {
        // this must be done before visiting the functions
        self.encode_types(module);
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