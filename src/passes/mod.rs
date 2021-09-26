#[cfg(feature = "opt")]
mod instr_rewrite;
#[cfg(feature = "opt")]
mod peephole_opt;

#[cfg(feature = "opt")]
pub use instr_rewrite::{InstrRewritePass, BlobRewriteData};
#[cfg(feature = "opt")]
pub use peephole_opt::PeepholeOpt;