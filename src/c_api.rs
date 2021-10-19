//! Offers C bindings to the library
#![allow(clippy::missing_safety_doc)]

use std::{ffi::CStr, panic::catch_unwind, ptr::null};

use crate::{builder::{self, FunctionBuilder, InstrBuilder}, instr::{self, BlockTag, Cmp}, irprint::IRPrint, module::{ExternFunction, Module, WasmModuleConf}, ty::{Ty, Type}};

#[inline]
fn c_alloc<T>(x: T) -> *mut () { Box::leak(Box::new(x)) as *mut T as *mut () }

#[inline]
unsafe fn c_dealloc<T>(x: *mut ()) { std::mem::drop(Box::from_raw(x as *mut T)) }

#[inline]
unsafe fn slice_of<T>(ptr: *const T, len: usize) -> &'static [T] {
    std::slice::from_raw_parts(ptr, len)
} 

#[inline]
unsafe fn string_of(ptr: *const i8) -> String {
    CStr::from_ptr(ptr).to_str().unwrap().to_string()
}

/// Take a value out of a pointer and replace it with zeros
#[inline]
unsafe fn take<T>(ptr: *mut T) -> T {
    let instance = std::ptr::read(ptr);
    std::ptr::write_bytes(ptr as *mut u8, 0, std::mem::size_of::<T>());
    instance
}

//
//--- HERE STARTS THE PUBLIC API ---
//

pub type ModuleRef = *mut ();

#[no_mangle]
pub extern "C" fn create_module() -> ModuleRef {
    c_alloc(Module::new(WasmModuleConf::default()))
}

#[no_mangle]
pub unsafe extern "C" fn free_module(module: ModuleRef) {
    c_dealloc::<Module>(module);
}

#[no_mangle]
pub unsafe extern "C" fn dump_module(module: ModuleRef) {
    let mut s = String::new();
    IRPrint::ir_print((module as *const Module).as_ref().unwrap(), &mut s).unwrap();
    eprint!("{}", s);
}

pub type TypeRef = *const ();

#[no_mangle]
pub unsafe extern "C" fn module_get_int32_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.int32t().as_ref() as *const Type as _)
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_uint32_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.uint32t().as_ref() as *const Type as _)
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_int16_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.int16t().as_ref() as *const Type as _)
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_uint16_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.uint16t().as_ref() as *const Type as _)
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_int8_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.int8t().as_ref() as *const Type as _)
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_uint8_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.uint8t().as_ref() as *const Type as _)
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_float32_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.float32t().as_ref() as *const Type as *const ())
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_ptr_type(module: ModuleRef) -> TypeRef {
    (module as *const Module).as_ref()
        .map(|m| m.ptr_t().as_ref() as *const Type as *const ())
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_func_type(module: ModuleRef, arg_types: *const TypeRef, argc: usize, ret_types: *const TypeRef, retc: usize) -> TypeRef {
    let args = slice_of(arg_types, argc).iter().map(|type_ref| {
        Ty::from_raw(*type_ref as *const () as *const Type)
    });
    let rets = slice_of(ret_types, retc).iter().map(|type_ref| {
        Ty::from_raw(*type_ref as *const () as *const Type)
    });
    (module as *mut Module).as_mut()
        .map(|m| m.intern_type(Type::Func {
            args: args.collect(),
            ret: rets.collect()
        }).as_ref() as *const Type as *const ())
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_get_struct_type(module: ModuleRef, field_types: *const TypeRef, fieldc: usize) -> TypeRef {
    let fields = slice_of(field_types, fieldc).iter().map(|type_ref| {
        Ty::from_raw(*type_ref as *const () as *const Type)
    });
    (module as *mut Module).as_mut()
        .map(|m| m.intern_type(Type::Struct {
            fields: fields.collect(),
        }).as_ref() as *const Type as *const ())
        .unwrap_or(null())
}

#[no_mangle]
pub unsafe extern "C" fn module_new_int_global(module: ModuleRef, global_name: *const i8, value: i32) {
    (module as *mut Module).as_mut().unwrap()
        .new_int_global(string_of(global_name), value);
}

#[no_mangle]
pub unsafe extern "C" fn module_new_float_global(module: ModuleRef, global_name: *const i8, value: f32) {
    (module as *mut Module).as_mut().unwrap()
        .new_float_global(string_of(global_name), value);
}

#[no_mangle]
pub unsafe extern "C" fn module_new_extern_function(
    module: ModuleRef, 
    function_name: *const i8, 
    function_type: TypeRef) {

    let func_name = string_of(function_name);
    let func_ty = Ty::from_raw(function_type as *const () as *const Type);
    (module as *mut Module).as_mut().unwrap().add_extern_function(ExternFunction::new(
        func_name, func_ty
    ))
}

pub type FunctionBuilderRef = *mut ();

#[no_mangle]
pub unsafe extern "C" fn create_function_builder(
    function_name: *const i8,
    function_type: TypeRef
) -> FunctionBuilderRef {
    let func_name = string_of(function_name);
    let func_ty = Ty::from_raw(function_type as *const () as *const Type);
    let (arguments, returns) = match &*func_ty {
        Type::Func { args, ret } => (args.clone(), ret.clone()),
        _ => panic!()
    };
    c_alloc(FunctionBuilder::new(func_name, arguments, returns))
}

#[no_mangle]
pub unsafe extern "C" fn finish_function_builder(module: ModuleRef, builder: FunctionBuilderRef) {
    let builder = take(builder as *mut FunctionBuilder);
    builder.finish((module as *mut Module).as_mut().unwrap());
}

pub type LocalRef = builder::LocalRef;

#[no_mangle]
pub unsafe extern "C" fn builder_get_arg(builder: FunctionBuilderRef, arg_index: usize) -> LocalRef {
    (builder as *const FunctionBuilder).as_ref().unwrap().get_arg(arg_index)
}

#[no_mangle]
pub unsafe extern "C" fn builder_new_local(builder: FunctionBuilderRef, ty: TypeRef) -> LocalRef {
    (builder as *mut FunctionBuilder).as_mut().unwrap().new_local(Ty::from_raw(ty as *const () as *const Type))
}

pub type BlockId = instr::BlockId;

#[no_mangle]
pub unsafe extern "C" fn builder_new_block(builder: FunctionBuilderRef, block_returns: *const TypeRef, block_returnc: usize, block_tag: BlockTag) -> BlockId {
    let returns = slice_of(block_returns, block_returnc).iter().map(|type_ref| {
        Ty::from_raw(*type_ref as *const () as *const Type)
    });
    (builder as *mut FunctionBuilder).as_mut().unwrap().new_block(returns, block_tag)
}

#[no_mangle]
pub unsafe extern "C" fn builder_switch_block(builder: FunctionBuilderRef, new_block: BlockId) {
    (builder as *mut FunctionBuilder).as_mut().unwrap().switch_block(new_block)
}

#[no_mangle]
pub unsafe extern "C" fn builder_get_current_block(builder: FunctionBuilderRef) -> BlockId {
    (builder as *mut FunctionBuilder).as_mut().unwrap().get_current_block()
}

// INSTRUCTIONS

#[no_mangle]
pub unsafe extern "C" fn builder_i_ld_int(builder: FunctionBuilderRef, val: u32, int_type: TypeRef) {
     (builder as *mut FunctionBuilder).as_mut().unwrap()
        .i_ld_int(val, Ty::from_raw(int_type as *const Type)) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_ld_float(builder: FunctionBuilderRef, val: f32) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_ld_float(val) }

macro_rules! argless_instr {
    ( $( $out_name:ident : $instr_name:ident )* ) => {
        $(       
#[no_mangle]
pub unsafe extern "C" fn $out_name(builder: FunctionBuilderRef) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().$instr_name() 
}
        )*
    };
}

argless_instr!(
    builder_i_iadd : i_iadd
    builder_i_isub : i_isub
    builder_i_imul : i_imul
    builder_i_idiv : i_idiv
    builder_i_fadd : i_fadd
    builder_i_fsub : i_fsub
    builder_i_fmul : i_fmul
    builder_i_fdiv : i_fdiv
    builder_i_itof : i_itof
    builder_i_call_indirect : i_call_indirect
    builder_i_end : i_end
    builder_i_memory_grow : i_memory_grow
    builder_i_memory_size : i_memory_size
    builder_i_discard : i_discard
    builder_i_return : i_return
);

#[no_mangle]
pub unsafe extern "C" fn builder_i_ftoi(builder: FunctionBuilderRef, int_type: TypeRef) {
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_ftoi(
        Ty::from_raw(int_type as *const Type)
    )
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_iconv(builder: FunctionBuilderRef, int_type: TypeRef) {
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_iconv(
        Ty::from_raw(int_type as *const Type)
    )
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_icmp(builder: FunctionBuilderRef, cmp: Cmp) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_icmp(cmp) }

#[no_mangle]
pub unsafe extern "C" fn builder_i_fcmp(builder: FunctionBuilderRef, cmp: Cmp) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_fcmp(cmp) }

#[no_mangle]
pub unsafe extern "C" fn builder_i_call(builder: FunctionBuilderRef, func_name: *const i8) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_call(string_of(func_name)) }

#[no_mangle]
pub unsafe extern "C" fn builder_i_ld_local(builder: FunctionBuilderRef, loc: LocalRef) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_ld_local(loc) }

#[no_mangle]
pub unsafe extern "C" fn builder_i_st_local(builder: FunctionBuilderRef, loc: LocalRef) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_st_local(loc) }

#[no_mangle]
pub unsafe extern "C" fn builder_i_ld_global_func(builder: FunctionBuilderRef, func_name: *const i8) { (builder as *mut FunctionBuilder).as_mut().unwrap().i_ld_global_func(string_of(func_name)) }

#[no_mangle]
pub unsafe extern "C" fn builder_i_bitcast(builder: FunctionBuilderRef, target_type: TypeRef) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_bitcast(
        Ty::from_raw(target_type as *const () as *const Type)
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_if(builder: FunctionBuilderRef, then_block: BlockId) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_if_else(
        then_block, None
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_if_else(builder: FunctionBuilderRef, then_block: BlockId, else_block: BlockId) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_if_else(
        then_block, Some(else_block)
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_read(builder: FunctionBuilderRef, ty: TypeRef) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_read(
        Ty::from_raw(ty as *const () as *const Type)
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_write(builder: FunctionBuilderRef, ty: TypeRef) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_write(
        Ty::from_raw(ty as *const () as *const Type)
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_offset(builder: FunctionBuilderRef, ty: TypeRef) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_offset(
        Ty::from_raw(ty as *const () as *const Type)
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_get_field_ptr(builder: FunctionBuilderRef, struct_ty: TypeRef, field_idx: usize) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_get_field_ptr(
        Ty::from_raw(struct_ty as *const () as *const Type),
        field_idx
    ) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_ld_global(builder: FunctionBuilderRef, name: *const i8) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_ld_global(string_of(name)) 
}

#[no_mangle]
pub unsafe extern "C" fn builder_i_st_global(builder: FunctionBuilderRef, name: *const i8) { 
    (builder as *mut FunctionBuilder).as_mut().unwrap().i_st_global(string_of(name)) 
}

#[no_mangle]
pub unsafe extern "C" fn compile_full_module(module: ModuleRef, opt: bool, out_len: *mut usize) -> *const u8 {
    let result = catch_unwind(|| {
        crate::pipeline_compile_module_to_wasm(take(module as *mut Module), opt)
    });
    match result {
        Ok(vec) => {
            std::ptr::write(out_len, vec.len());
            vec.leak().as_ptr()
        }
        Err(_) => null()
    }
}
