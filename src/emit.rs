use std::{collections::HashMap, convert::TryInto};

use wasm_encoder as wasm;

use crate::{instr::{Cmp, Function, InstrK}, module::Module, pass::FunctionPass, ty::{Ty, Type}};

pub struct WasmEmitter<'ctx> {
    module: wasm::Module,
    /// A table of function types and their indexes in the result wasm module
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
    code_sec: wasm::CodeSection
}

impl<'ctx> WasmEmitter<'ctx> {
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
        }
    }

    fn encode_types(&mut self, module: &Module<'ctx>) {
        for ty in module.all_types_iter() {
            if let Type::Func { args, ret } = &*ty {
                self.type_sec.function(
                    args.iter().map(|t| Self::get_wasm_type(*t)),
                    ret.iter().map(|t| Self::get_wasm_type(*t)) 
                );
                // The function type is the last one
                let idx = self.type_sec.len() - 1;
                self.function_types.insert(ty, idx);
            }
        }
    }

    // Not using &self prevents borrow errors, and we don't actually need t
    fn get_wasm_type(ty: Ty<'ctx>) -> wasm::ValType {
        match &*ty {
            Type::Int32 => wasm::ValType::I32,
            Type::Float32 => wasm::ValType::F32,
            Type::Func { args: _, ret: _ } => wasm::ValType::I32, /*wasm::ValType::FuncRef /* FIXME: not sure if this is correct */*/
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
                .map(|t| Self::get_wasm_type(*t)));
        
        let mut out_f = wasm::Function::new(local_iter);

        for instr in &func.body().body {
            match &instr.kind {
                InstrK::LdInt(val) => out_f.instruction(wasm::Instruction::I32Const(*val)),
                InstrK::LdFloat(val) => out_f.instruction(wasm::Instruction::F32Const(*val)),
                InstrK::IAdd => out_f.instruction(wasm::Instruction::I32Add),
                InstrK::ISub => out_f.instruction(wasm::Instruction::I32Sub),
                InstrK::IMul => out_f.instruction(wasm::Instruction::I32Mul),
                InstrK::IDiv => out_f.instruction(wasm::Instruction::I32DivS),
                InstrK::FAdd => out_f.instruction(wasm::Instruction::F32Add),
                InstrK::FSub => out_f.instruction(wasm::Instruction::F32Sub),
                InstrK::FMul => out_f.instruction(wasm::Instruction::F32Mul),
                InstrK::FDiv => out_f.instruction(wasm::Instruction::F32Div),
                InstrK::Itof => out_f.instruction(wasm::Instruction::F32ConvertI32S),
                InstrK::Ftoi => out_f.instruction(wasm::Instruction::I32TruncSatF32S),
                InstrK::ICmp(cmp) => match cmp {
                    Cmp::Eq => out_f.instruction(wasm::Instruction::I32Eq),
                    Cmp::Ne => out_f.instruction(wasm::Instruction::I32Neq),
                    Cmp::Lt => out_f.instruction(wasm::Instruction::I32LtS),
                    Cmp::Le => out_f.instruction(wasm::Instruction::I32LeS),
                    Cmp::Gt => out_f.instruction(wasm::Instruction::I32GtS),
                    Cmp::Ge => out_f.instruction(wasm::Instruction::I32GeS),
                }
                InstrK::FCmp(cmp) => match cmp {
                    Cmp::Eq => out_f.instruction(wasm::Instruction::F32Eq),
                    Cmp::Ne => out_f.instruction(wasm::Instruction::F32Neq),
                    Cmp::Lt => out_f.instruction(wasm::Instruction::F32Lt),
                    Cmp::Le => out_f.instruction(wasm::Instruction::F32Le),
                    Cmp::Gt => out_f.instruction(wasm::Instruction::F32Gt),
                    Cmp::Ge => out_f.instruction(wasm::Instruction::F32Ge),
                }
                InstrK::CallDirect { func_name } => {
                    let func_idx = module.get_function(func_name).unwrap().idx;
                    out_f.instruction(wasm::Instruction::Call(func_idx.try_into().unwrap()))
                },
                InstrK::Return => out_f.instruction(wasm::Instruction::Return),
                InstrK::LdLocal { idx } => out_f.instruction(wasm::Instruction::LocalGet(*idx as u32)),
                InstrK::StLocal { idx } => out_f.instruction(wasm::Instruction::LocalSet(*idx as u32)),
                InstrK::LdGlobalFunc { func_name } => {
                    let func_idx = module.get_function(func_name).unwrap().idx;
                    // the index must be shifted by one - see the description of [`emit_global_function_table`]
                    out_f.instruction(wasm::Instruction::I32Const((func_idx + 1).try_into().unwrap()))
                },
                InstrK::CallIndirect => {
                    // this is injected by the Verifier
                    let function_ty = instr.meta.retrieve_ty("ty").unwrap();
                    println!("{:?}", function_ty);
                    out_f.instruction(wasm::Instruction::CallIndirect {
                        ty: self.function_types[&function_ty],
                        table: 0 // the GFT is the only one and it's at index zero
                    })
                },
            };
        }

        out_f.instruction(wasm::Instruction::End); // every function body must end with the End instr.

        // Then add to the sections
        // first the function section
        self.func_sec.function(self.function_types[&func.ty()]);
        // then the export section
        // TODO: specify whether the function should be exported
        self.export_sec.export(func.name(), wasm::Export::Function(func.idx as u32));
        // then the code section
        self.code_sec.function(&out_f);
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

        // An active element section initializes the table at start
        let mut elements_vec = vec![wasm::Element::Null]; // the first element in the table (at index zero) is a null ref
        elements_vec.extend(
            (0..(table_length-1)).map(wasm::Element::Func));
        self.elem_sec.active(
            Some(0), 
            wasm::Instruction::I32Const(0), 
            wasm::ValType::FuncRef, 
            wasm::Elements::Expressions(&elements_vec));
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

impl<'ctx> FunctionPass<'ctx> for WasmEmitter<'ctx> {
    type Error = (); // TODO some error

    fn visit_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> {
        // this must be done before visiting the functions
        self.encode_types(module);
        println!("{:?}", self.function_types);
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