use std::cell::{RefCell};

use indexmap::IndexMap;
use libintern::Interner;

use crate::{instr::Function, irprint::IRPrint, pass::{FunctionPass, MutableFunctionPass}, ty::{Ty, Type}};

pub struct Module<'ctx> {
    // this is not true anymore:
    // The type context is ref-celled mainly for reasons of simplicity
    // to allow interning a type while e.g. modifying a function
    type_ctx: RefCell<Interner<'ctx, Type<'ctx>>>,
    // IndexMap ensures the functions (and globals) have a constant index (ordering)
    functions: IndexMap<String, FuncDef<'ctx>>,
    globals: IndexMap<String, Global<'ctx>>,
    /// We cache Ty<'ctx> of primitive types for faster access
    primitive_types_cache: PrimitiveTypeCache<'ctx>,
    /// Some configuration of the result webassembly module
    pub conf: WasmModuleConf
}

/// Configuration of the webassembly module
pub struct WasmModuleConf {
    /// The initial WebAssembly memory size in units of pages
    pub initial_memory_size: u32,
    /// If true, the Float-to-int conversions will be saturating
    /// Otherwise, they will trap on unexpected values
    ///
    /// For more details, see the WebAssembly documentation on `iNN.trunc_fNN` and `iNN.trunc_sat_fNN`.
    pub use_saturating_ftoi: bool,
}

impl Default for WasmModuleConf {
    fn default() -> Self {
        WasmModuleConf { initial_memory_size: 1, use_saturating_ftoi: true }
    }
}

struct PrimitiveTypeCache<'ctx> {
    int32: Ty<'ctx>,
    uint32: Ty<'ctx>,
    float32: Ty<'ctx>,
    ptr: Ty<'ctx>,
    int16: Ty<'ctx>,
    uint16: Ty<'ctx>,
    int8: Ty<'ctx>,
    uint8: Ty<'ctx>,
}

impl Default for Module<'_> {
    fn default() -> Self {
        Self::new(WasmModuleConf::default())
    }
}

impl<'ctx> Module<'ctx> {
    pub fn new(wasm_module_conf: WasmModuleConf) -> Self {
        let mut type_ctx = Interner::new();
        let cache = PrimitiveTypeCache {
            int32: type_ctx.intern(Type::Int32),
            uint32: type_ctx.intern(Type::UInt32),
            float32: type_ctx.intern(Type::Float32),
            ptr: type_ctx.intern(Type::Ptr),
            int16: type_ctx.intern(Type::Int16),
            uint16: type_ctx.intern(Type::UInt16),
            int8: type_ctx.intern(Type::Int8),
            uint8: type_ctx.intern(Type::UInt8),
        };
        Module {
            type_ctx: RefCell::new(type_ctx),
            functions: IndexMap::new(),
            globals: IndexMap::new(),
            primitive_types_cache: cache,
            conf: wasm_module_conf
        }
    }

    pub fn intern_type(&self, ty: Type<'ctx>) -> Ty<'ctx> {
        self.type_ctx.borrow_mut().intern(ty)
    }

    pub fn for_all_types_iter(&self, mut f: impl FnMut(Ty<'ctx>))  {
        for t in self.type_ctx.borrow().iter() {
            f(t)
        }
    }

    pub fn add_function(&mut self, mut function: Function<'ctx>) {
        if self.functions.contains_key(function.name()) {
            panic!("Multiple functions with the same name") // TODO better handle
        }
        
        // set the function index
        function.idx = self.functions.len();
        // clone its name
        let cloned_name = function.name().to_owned();
        // save it
        self.functions.insert(cloned_name, FuncDef::Local(function));
    }

    /// Return an immutable reference to a Function.
    /// Returns None if the function doesn't exist.
    pub fn get_function(&self, name: &str) -> Option<&FuncDef<'ctx>> {
        self.functions.get(name)
    }

    pub fn functions_iter(&self) -> impl Iterator<Item = &FuncDef<'ctx>> {
        self.functions.values()
    }

    pub(crate) fn function_count(&self) -> usize {
        self.functions.len()
    }

    pub(crate) fn function_get_by_idx(&self, idx: usize) -> &FuncDef<'ctx> {
        self.functions.get_index(idx).unwrap().1
    }

    pub(crate) fn function_get_mut_by_idx(&mut self, idx: usize) -> &mut FuncDef<'ctx> {
        self.functions.get_index_mut(idx).unwrap().1
    }

    /// Print the IR of this module to stdout
    pub fn dump_module(&self) {
        let mut s = String::new();
        self.ir_print(&mut s).unwrap();
        print!("{}", s);
    }

    pub fn int32t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.int32
    }

    pub fn uint32t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.uint32
    }

    pub fn int16t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.int16
    }

    pub fn uint16t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.uint16
    }

    pub fn int8t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.int8
    }

    pub fn uint8t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.uint8
    }

    pub fn float32t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.float32
    }

    pub fn ptr_t(&self) -> Ty<'ctx> {
        self.primitive_types_cache.ptr
    }

    pub fn do_pass<P: FunctionPass<'ctx>>(&self, passer: &mut P) -> Result<(), P::Error> {
        passer.visit_module(self)?;
        for func_def in self.functions.values() {
            if let FuncDef::Local(func) = func_def {
                passer.visit_function(self, func)?;
            }
        }
        passer.end_module(self)?;
        Ok(())
    }

    pub fn do_mut_pass<P: MutableFunctionPass<'ctx>>(&mut self, passer: &mut P) -> Result<(), P::Error> {
        passer.visit_module(self)?;
        for i in 0..self.functions.len() {
            if self.functions[i].is_local() {
                let info = passer.visit_function(self, self.functions[i].unwrap_local())?;
                passer.mutate_function(self.functions[i].unwrap_local_mut(), info)?;
            }
        }
        Ok(())
    }

    /// Create a new global of an integer type
    pub fn new_int_global(&mut self, name: String, value: i32) {
        // TODO: handle two globals with the same name
        let global = Global { name, ty: self.int32t(), value: GlobalValueInit::ConstInt(value), idx: 0 };
        self.new_global(global)
    }

    /// Create a new global of a floating-point type
    pub fn new_float_global(&mut self, name: String, value: f32) {
        let global = Global { name, ty: self.float32t(), value: GlobalValueInit::ConstFloat(value), idx: 0 };
        self.new_global(global)
    }

    fn new_global(&mut self, mut g: Global<'ctx>) {
        let idx = self.globals.len();
        g.idx = idx;
        self.globals.insert(g.name.clone(), g);
    }

    pub fn globals_iter(&self) -> impl Iterator<Item = &Global<'ctx>> {
        self.globals.values()
    }

    pub fn get_global(&self, name: &str) -> Option<&Global<'ctx>> {
        self.globals.get(name)
    }

    /// Add a new external function definition.
    /// 
    /// **All external functions must be defined before ANY local functions**.
    /// This ensures correct indexing during WASM compilation.
    pub fn add_extern_function(&mut self, mut function: ExternFunction<'ctx>) {
        if self.functions.contains_key(function.name()) {
            panic!("Multiple functions with the same name") // TODO better handle
        }
        if self.functions.values().any(|def| def.is_local()) {
            panic!("All extern functions must be defined before any local functions")
        }
        
        // set the function index
        function.idx = self.functions.len();
        // clone its name
        let cloned_name = function.name().to_owned();
        // save it
        self.functions.insert(cloned_name, FuncDef::Extern(function));
    }
}

pub struct Global<'ctx> {
    pub(crate) name: String,
    pub(crate) ty: Ty<'ctx>,
    value: GlobalValueInit,
    /// The Global's index (equivalent to how functions have indexes)
    /// assigned by the module
    idx: usize
}

impl<'ctx> Global<'ctx> {
    pub(crate) fn is_int(&self) -> bool {
        matches!(self.value, GlobalValueInit::ConstInt(_))
    }

    pub(crate) fn is_float(&self) -> bool {
        matches!(self.value, GlobalValueInit::ConstFloat(_))
    }

    pub(crate) fn get_int_value(&self) -> i32 {
        match self.value {
            GlobalValueInit::ConstInt(x) => x,
            _ => panic!()
        }
    }

    pub(crate) fn get_float_value(&self) -> f32 {
        match self.value {
            GlobalValueInit::ConstFloat(x) => x,
            _ => panic!()
        }
    }

    pub(crate) fn idx(&self) -> usize { self.idx }
}

enum GlobalValueInit {
    ConstInt(i32),
    ConstFloat(f32),
    // TODO: ConstFunc (and other types)
}

pub struct ExternFunction<'ctx> {
    name: String,
    ty: Ty<'ctx>,
    idx: usize,
}

impl<'ctx> ExternFunction<'ctx> {
    pub fn new(name: String, ty: Ty<'ctx>) -> Self {
        assert!(ty.is_func(), "The type of a Function must be a function type");

        ExternFunction { name, ty, idx: usize::MAX }
    }

    pub fn ret_tys(&self) -> &Vec<Ty<'ctx>> {
        match &*self.ty {
            Type::Func { args: _, ret } => ret,
            _ => unreachable!()
        }
    }

    pub fn arg_tys(&self) -> &Vec<Ty<'ctx>> {
        match &*self.ty {
            Type::Func { args, ret: _ } => args,
            _ => unreachable!()
        } 
    }
}

pub enum FuncDef<'ctx> {
    Local(Function<'ctx>),
    Extern(ExternFunction<'ctx>)
}

impl<'ctx> FuncDef<'ctx> {
    pub fn is_local(&self) -> bool { matches!(self, FuncDef::Local(_)) }
    pub fn is_extern(&self) -> bool { matches!(self, FuncDef::Extern(_)) }
    pub fn unwrap_local(&self) -> &Function<'ctx> { 
        match self {
            FuncDef::Local(f) => f,
            FuncDef::Extern(_) => panic!(),
        } 
    }
    pub fn unwrap_local_mut(&mut self) -> &mut Function<'ctx> { 
        match self {
            FuncDef::Local(f) => f,
            FuncDef::Extern(_) => panic!(),
        } 
    }
}

/// A trait implemented for both [`Function`] and [`ExternFunction`]
pub trait Functional<'ctx> : IRPrint + 'ctx {
    /// Return the name of the function
    fn name(&self) -> &str;
    /// Return the type of the function
    fn ty(&self) -> Ty<'ctx>;
    /// Return the index of the function
    fn idx(&self) -> usize;
    fn arg_tys(&self) -> &Vec<Ty<'ctx>>;
    fn ret_tys(&self) -> &Vec<Ty<'ctx>>;
}

impl<'ctx> Functional<'ctx> for Function<'ctx> {
    fn name(&self) -> &str { self.name() }
    fn ty(&self) -> Ty<'ctx> { self.ty() }
    fn idx(&self) -> usize { self.idx }
    fn arg_tys(&self) -> &Vec<Ty<'ctx>> { self.arg_tys() }
    fn ret_tys(&self) -> &Vec<Ty<'ctx>> { self.ret_tys() }
}

impl<'ctx> Functional<'ctx> for ExternFunction<'ctx> {
    fn name(&self) -> &str { &self.name }
    fn ty(&self) -> Ty<'ctx> { self.ty }
    fn idx(&self) -> usize { self.idx }
    fn arg_tys(&self) -> &Vec<Ty<'ctx>> { self.arg_tys() }
    fn ret_tys(&self) -> &Vec<Ty<'ctx>> { self.ret_tys() }
}

impl<'ctx> Functional<'ctx> for FuncDef<'ctx> {
    fn name(&self) -> &str { 
        match &self {
            FuncDef::Local(f) => f.name(),
            FuncDef::Extern(f) => f.name(),
        }
    }

    fn ty(&self) -> Ty<'ctx> {
        match &self {
            FuncDef::Local(f) => f.ty(),
            FuncDef::Extern(f) => f.ty(),
        }
    }

    fn idx(&self) -> usize {
        match &self {
            FuncDef::Local(f) => f.idx(),
            FuncDef::Extern(f) => f.idx(),
        }
    }

    fn arg_tys(&self) -> &Vec<Ty<'ctx>> {
        match &self {
            FuncDef::Local(f) => f.arg_tys(),
            FuncDef::Extern(f) => f.arg_tys(),
        }
    }

    fn ret_tys(&self) -> &Vec<Ty<'ctx>> {
        match &self {
            FuncDef::Local(f) => f.ret_tys(),
            FuncDef::Extern(f) => f.ret_tys(),
        }
    }
}