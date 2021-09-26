mod instr_rewrite;
mod peephole_opt;

pub use instr_rewrite::{InstrRewritePass, BlobRewriteData};
pub use peephole_opt::PeepholeOpt;