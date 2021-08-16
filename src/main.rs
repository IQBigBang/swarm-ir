#![feature(bench_black_box)] // TODO remove

use builder::FunctionBuilder;
use module::Module;

use crate::instr::Cmp;
use crate::irprint::IRPrint;
use crate::{emit::WasmEmitter, verify::Verifier};
use crate::module::WasmModuleConf;

pub mod ty;
pub mod instr;
pub mod metadata;
pub mod module;
pub mod pass;
pub mod verify;
pub mod emit;
pub mod builder;
pub mod irprint;

fn main() {
    /*let mut top = Module::new(WasmModuleConf::default());

    let mut block = InstrBlock::new();
    /*block.add(InstrK::LdInt(2));
    block.add(InstrK::Itof);
    block.add(InstrK::LdFloat(3.0));
    block.add(InstrK::FAdd);
    block.add(InstrK::Return);*/
    /*block.add(InstrK::LdLocal { idx: 0 });
    block.add(InstrK::LdLocal { idx: 1 });
    block.add(InstrK::IAdd);
    block.add(InstrK::Return);*/
    block.add(InstrK::LdLocal { idx: 0 });
    block.add(InstrK::LdInt(1));
    block.add(InstrK::IAdd);
    block.add(InstrK::Return);

    let func_ty = top.intern_type(ty::Type::Func { args: vec![
        top.int32t()
    ], ret: top.int32t() });

    let func = Function::new("add_one".to_string(), func_ty, block, []);

    top.add_function(func);

    let mut block2 = InstrBlock::new();
    block2.add(InstrK::LdLocal { idx: 0 });
    block2.add(InstrK::LdInt(2));
    block2.add(InstrK::IAdd);
    block2.add(InstrK::Return);
    let func2 = Function::new("add_two".to_string(), func_ty, block2, []);
    top.add_function(func2);

    let mut block3 = InstrBlock::new();
    block3.add(InstrK::LdLocal { idx: 0 });
    block3.add(InstrK::LdInt(3));
    block3.add(InstrK::IAdd);
    block3.add(InstrK::Return);
    let func3 = Function::new("add_three".to_string(), func_ty, block3, []);
    top.add_function(func3);

    let no_arg_func_ty = top.intern_type(ty::Type::Func { args: vec![], ret: top.int32t() });
    let mut block4 = InstrBlock::new();
    block4.add(InstrK::LdInt(4));
    block4.add(InstrK::LdGlobalFunc { func_name: "add_two".to_string() });
    block4.add(InstrK::CallIndirect);
    block4.add(InstrK::Return);
    let func4 = Function::new("do_smth".to_string(), no_arg_func_ty, block4, []);
    top.add_function(func4);

    top.do_mut_pass(&mut Verifier{}).unwrap_or_else(|_e| panic!("Verify error"));

    let mut e = WasmEmitter::new();
    top.do_pass(&mut e).unwrap();

    let result_wasm = e.finish();
    std::fs::write("output.wasm", &result_wasm).unwrap();
    println!("{}", wasmprinter::print_bytes(&result_wasm).unwrap());

    let exec_mod = wasmi::Module::from_buffer(&result_wasm).unwrap();
    let instance = 
        ModuleInstance::new(&exec_mod, &ImportsBuilder::default()).unwrap()
        .assert_no_start();
    
    let result_val = instance.invoke_export("f", &[
        RuntimeValue::I32(13), RuntimeValue::I32(16)
    ], &mut NopExternals).unwrap();

    println!("{:?}", result_val);*/

    use crate::builder::InstrBuilder;

    let mut top = Module::new(WasmModuleConf::default());

    let mut builder = FunctionBuilder::new(
        "add_one".to_string(),
        [top.int32t()],
        [top.int32t()]
    );
    let arg0 = builder.get_arg(0);
    builder.i_ld_local(arg0);
    builder.i_ld_int(1);
    builder.i_iadd();
    builder.i_return();

    builder.finish(&mut top);

    builder = FunctionBuilder::new(
        "test1".to_string(),
        [],
        [top.int32t()]
    );
    builder.i_ld_int(20);
    builder.i_call("add_one".to_string());
    builder.i_return();

    builder.finish(&mut top);

    builder = FunctionBuilder::new(
        "test2".to_string(),
        [],
        [top.int32t()]
    );
    builder.i_ld_int(20);
    builder.i_ld_global_func("add_one".to_string());
    builder.i_call_indirect();
    builder.i_return();

    builder.finish(&mut top);

    builder = FunctionBuilder::new(
        "cmp_test".to_string(),
        [],
        [top.int32t()]
    );
    builder.i_ld_float(15.0);
    builder.i_ld_float(-2.0);
    builder.i_fcmp(Cmp::Gt);
    builder.i_ld_int(1);
    builder.i_iadd();
    builder.i_return();

    builder.finish(&mut top);

    builder = FunctionBuilder::new(
        "bitcast_test".to_string(),
        [],
        [top.int32t()]
    );

    builder.i_ld_global_func("test1".to_string());
    builder.i_bitcast(top.int32t());
    builder.i_ld_int(1);
    builder.i_iadd();
    builder.i_bitcast(top.get_function("test2").unwrap().ty());
    builder.i_call_indirect();
    builder.i_return();
    builder.finish(&mut top);

    let mut s = String::new();
    top.ir_print(&mut s).unwrap();
    print!("{}", s);

    top.do_mut_pass(&mut Verifier{}).unwrap_or_else(|_e| panic!("Verify error {:?}", _e));

    let mut s = String::new();
    top.ir_print(&mut s).unwrap();
    print!("{}", s);

    let mut e = WasmEmitter::new();
    top.do_pass(&mut e).unwrap();

    let result_wasm = e.finish();
    std::fs::write("output.wasm", &result_wasm).unwrap();
    println!("{}", wasmprinter::print_bytes(&result_wasm).unwrap());

}
