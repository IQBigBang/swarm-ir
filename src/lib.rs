pub mod ty;
pub mod instr;
// This module doesn't need to be public as it doesn't contain anything public anyway
#[macro_use]
pub(crate) mod metadata;
pub mod module;
pub mod pass;
pub mod verify;
pub mod emit;
pub mod builder;
pub mod irprint;
pub mod correct;
pub mod cf_verify;
pub mod abi;
pub mod passes;
#[cfg(feature = "c-api")]
pub mod c_api;
pub mod intrinsic;
#[cfg(feature = "ir-parse")]
pub mod irparse;
pub mod numerics;
pub mod staticmem;

/// Run the standard pipeline of passes on an IR module
/// with the exception of the last pass - the compilation.
/// 
/// Panics if any kind of error during verification occurs
pub fn pipeline_verify_module(module: &mut module::Module<'_>) {
    module.do_mut_pass(&mut correct::CorrectionPass{}).unwrap();
    module.do_mut_pass(&mut cf_verify::ControlFlowVerifier{}).unwrap();
    module.do_mut_pass(&mut verify::Verifier{}).unwrap();
}

/// Compile an IR Module to WebAssembly with the default
/// preferred pipeline.
///
/// This is a simplification for users so that they don't have
/// to worry about invoking necessary passes in correct order.
///
/// Panics if any kind of error happens while compiling/verifying etc.
pub fn pipeline_compile_module_to_wasm(mut module: module::Module<'_>, opt: bool) -> Vec<u8> {
    use pass::{MutableFunctionPass, FunctionPass};

    module.do_mut_pass(&mut correct::CorrectionPass{}).unwrap();
    module.do_mut_pass(&mut cf_verify::ControlFlowVerifier{}).unwrap();
    module.do_mut_pass(&mut verify::Verifier{}).unwrap();

    #[cfg(feature = "opt")]
    if opt {
        for i in 0..module.function_count() {
            if module.function_get_by_idx(i).is_extern() { continue }
            let result = passes::PeepholeOpt{}.visit_function(&module, module.function_get_by_idx(i).unwrap_local()).unwrap();
            let mut rewrite_pass = passes::InstrRewritePass::new(i, result).unwrap();
            rewrite_pass.visit_function(&module, module.function_get_by_idx(i).unwrap_local()).unwrap();
            rewrite_pass.mutate_function(module.function_get_mut_by_idx(i).unwrap_local_mut(), ()).unwrap();
        }
    }

    let mut e: emit::WasmEmitter<abi::Wasm32Abi> = emit::WasmEmitter::new();
    module.do_pass(&mut e).unwrap();
    e.finish()
}