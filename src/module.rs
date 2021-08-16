use std::{collections::HashMap};

use libintern::Interner;

use crate::{instr::Function, pass::{FunctionPass, MutableFunctionPass}, ty::{Ty, Type}};

pub struct Module<'ctx> {
    // this is not true anymore:
    // The type context is ref-celled mainly for reasons of simplicity
    // to allow interning a type while e.g. modifying a function
    type_ctx: Interner<'ctx, Type<'ctx>>,
    // The functions are in a vector to make sure they have an ordering which does not change
    functions: Vec<Function<'ctx>>,
    // This is for fast lookup by name
    function_registry: HashMap<String, usize>,
    /// We cache Ty<'ctx> of primitive types for faster access
    primitive_types_cache: PrimitiveTypeCache<'ctx>,
    /// Some configuration of the result webassembly module
    pub conf: WasmModuleConf
}

/// Configuration of the webassembly module
pub struct WasmModuleConf {
    pub initial_memory_size: u32
}

impl Default for WasmModuleConf {
    fn default() -> Self {
        WasmModuleConf { initial_memory_size: 1 }
    }
}

struct PrimitiveTypeCache<'ctx> {
    int32: Ty<'ctx>,
    float32: Ty<'ctx>,
}

impl<'ctx> Module<'ctx> {
    pub fn new(wasm_module_conf: WasmModuleConf) -> Self {
        let mut type_ctx = Interner::new();
        let cache = PrimitiveTypeCache {
            int32: type_ctx.intern(Type::Int32),
            float32: type_ctx.intern(Type::Float32)
        };
        Module {
            type_ctx/*: RefCell::new(type_ctx)*/,
            functions: Vec::new(),
            function_registry: HashMap::new(),
            primitive_types_cache: cache,
            conf: wasm_module_conf
        }
    }

    pub fn intern_type(&mut self, ty: Type<'ctx>) -> Ty<'ctx> {
        self.type_ctx/*.borrow_mut()*/.intern(ty)
    }

    pub fn all_types_iter<'a>(&'a self) -> libintern::Iter<'a, 'ctx, Type<'ctx>> {
        self.type_ctx.iter()
    }

    pub fn add_function(&mut self, mut function: Function<'ctx>) {
        // TODO: handle if a function with the same name already exists
        
        // set the function index
        function.idx = self.functions.len();
        // clone its name
        let cloned_name = function.name().to_owned();
        // save it
        self.functions.push(function);
        // and save it into the map
        self.function_registry.insert(cloned_name, self.functions.len() - 1);
    }

    /// Return an immutable reference to a Function.
    /// Returns None if the function doesn't exist.
    pub fn get_function(&self, name: &str) -> Option<&Function<'ctx>> {
        let idx = *self.function_registry.get(name)?;
        Some(&self.functions[idx])
    }

    pub fn functions_iter(&self) -> std::slice::Iter<'_, Function<'ctx>> {
        self.functions.iter()
    }

    pub fn int32t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.int32
    }

    pub fn float32t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.float32
    }

    pub fn do_pass<P: FunctionPass<'ctx>>(&self, passer: &mut P) -> Result<(), P::Error> {
        passer.visit_module(self)?;
        for func in self.functions.iter() {
            passer.visit_function(self, func)?;
        }
        passer.end_module(self)?;
        Ok(())
    }

    pub fn do_mut_pass<P: MutableFunctionPass<'ctx>>(&mut self, passer: &mut P) -> Result<(), P::Error> {
        passer.visit_module(self)?;
        for i in 0..self.functions.len() {
            let info = passer.visit_function(self, &self.functions[i])?;
            passer.mutate_function(&mut self.functions[i], info)?;
        }
        Ok(())
    }
}