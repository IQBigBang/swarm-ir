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
pub fn pipeline_compile_module_to_wasm(mut module: module::Module<'_>) -> Vec<u8> {
    module.do_mut_pass(&mut correct::CorrectionPass{}).unwrap();
    module.do_mut_pass(&mut cf_verify::ControlFlowVerifier{}).unwrap();
    module.do_mut_pass(&mut verify::Verifier{}).unwrap();

    let mut e: emit::WasmEmitter<abi::Wasm32Abi> = emit::WasmEmitter::new();
    module.do_pass(&mut e).unwrap();
    e.finish()
}