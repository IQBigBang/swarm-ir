pub mod ty;
pub mod instr;
// This module doesn't need to be public as it doesn't contain anything public anyway
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
pub mod intrinsic;

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

    if opt {
        for i in 0..module.function_count() {
            let result = passes::PeepholeOpt{}.visit_function(&module, module.function_get_by_idx(i)).unwrap();
            let mut rewrite_pass = passes::InstrRewritePass::new(i, result).unwrap();
            rewrite_pass.visit_function(&module, module.function_get_by_idx(i)).unwrap();
            rewrite_pass.mutate_function(module.function_get_mut_by_idx(i), ()).unwrap();
        }
    }

    let mut e: emit::WasmEmitter<abi::Wasm32Abi> = emit::WasmEmitter::new();
    module.do_pass(&mut e).unwrap();
    e.finish()
}