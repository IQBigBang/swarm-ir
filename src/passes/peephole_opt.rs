use std::collections::HashMap;

use crate::{abi::{Abi, Wasm32Abi}, instr::{BlockId, Instr, InstrK}, intrinsic::Intrinsics, pass::{FunctionPass}, ty::{Ty, Type}};

use super::BlobRewriteData;

// TODO: this is just copied from the emitter code
fn calc_struct_field_offset(struct_ty: Ty, field_idx: usize) -> usize {
    let struct_fields = match &*struct_ty {
        Type::Struct { fields } => fields,
        _ => unreachable!()
    };
    <Wasm32Abi as Abi>::struct_field_offset(struct_fields, field_idx)
}

/// Replace two consecutive instructions with something new
fn replace_2<'ctx>(i1: &Instr<'ctx>, i2: &Instr<'ctx>) -> Option<Vec<Instr<'ctx>>> {
    match (&i1.kind, &i2.kind) {
        // [GetFieldPtr, Read] -> [ReadAtOffset]
        /*(InstrK::GetFieldPtr { struct_ty, field_idx }, InstrK::Read { ty }) => {
            let offset = calc_struct_field_offset(*struct_ty, *field_idx);
            Some(vec![Instr::new_intrinsic(Intrinsics::ReadAtOffset { offset, ty: *ty })])
        },*/
        // [LoadGlobalFunc, CallIndirect] -> [CallDirect]
        (InstrK::LdGlobalFunc { func_name }, InstrK::CallIndirect) => {
            // CallIndirect has the type metadata
            let meta = i2.meta.clone();
            Some(vec![Instr::new_with_meta(InstrK::CallDirect { func_name: func_name.clone() }, meta)])
        }
        _ => None
    }
}

/// Replace two consecutive instructions with something new
fn replace_3<'ctx>(i1: &Instr<'ctx>, i2: &Instr<'ctx>, i3: &Instr<'ctx>) -> Option<Vec<Instr<'ctx>>> {
    match (&i1.kind, &i2.kind, &i3.kind) {
        // [GetFieldPtr, load-instr, Write] -> [load-instr, WriteAtOffset]
        /*(InstrK::GetFieldPtr { struct_ty, field_idx }, _, InstrK::Write { ty }) => {
            if i2.is_load() {
                let offset = calc_struct_field_offset(*struct_ty, *field_idx);
                Some(vec![
                    i2.clone(),
                    Instr::new_intrinsic(Intrinsics::WriteAtOffset { offset, ty: *ty })])
            } else { None }
        }*/
        _ => None
    }
}

pub struct PeepholeOpt {}

impl<'ctx> FunctionPass<'ctx> for PeepholeOpt {
    type Error = ();
    // Returns a type suitable for InstrRewritePass
    type Output = HashMap<BlockId, Vec<BlobRewriteData<'ctx>>>;

    fn visit_function(
        &mut self, 
        module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<Self::Output, Self::Error> {
        
        let mut rewrite_data = HashMap::new();

        for block in function.blocks_iter() {
            let mut this_block_replacements: Vec<BlobRewriteData<'ctx>> = Vec::new();

            for i in 0..block.body.len() {
                // Check if there's 2 consecutive instructions left
                if (i + 1) < block.body.len() {
                    if let Some(new_instrs) = replace_2(&block.body[i], &block.body[i+1]) {
                        let range = i..(i + 2);
                        this_block_replacements.push((range, new_instrs));
                    }
                }
                // Check if there's 3 consecutive instructions left
                if (i + 2) < block.body.len() {
                    if let Some(new_instrs) = replace_3(&block.body[i], &block.body[i+1], &block.body[i+2]) {
                        let range = i..(i + 3);
                        this_block_replacements.push((range, new_instrs));
                    }
                }
                // TODO: replace 4 instructions etc.
            }

            if !this_block_replacements.is_empty() {
                rewrite_data.insert(block.idx, this_block_replacements);
            }
        }

        Ok(rewrite_data)
    }
}