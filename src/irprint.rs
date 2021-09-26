use crate::{instr::{BlockId, Cmp, Function, Instr, InstrBlock, InstrK}, module::Module, ty::{Ty, Type}};

pub trait IRPrint {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result;
}

impl<'ctx> IRPrint for Type<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        match self {
            Type::Int32 => write!(w, "int32"),
            Type::Float32 => write!(w, "float32"),
            Type::Ptr => write!(w, "ptr"),
            Type::Func { args, ret: rets } => {
                if args.is_empty() {
                    write!(w, "() -> ")?;
                } else {
                    write!(w, "(")?;
                    args[0].ir_print(w)?;
                    for arg in args.iter().skip(1) {
                        write!(w, ", ")?;
                        arg.ir_print(w)?;
                    }
                    write!(w, ") -> ")?;
                }

                if rets.is_empty() {
                    write!(w, "()")
                } else if rets.len() == 1 {
                    rets[0].ir_print(w) // no parantheses if there's only one return value
                } else {
                    write!(w, "(")?;
                    rets[0].ir_print(w)?;
                    for r in rets.iter().skip(1) {
                        write!(w, ", ")?;
                        r.ir_print(w)?;
                    }
                    write!(w, ")")
                }
            },
            Type::Struct { fields} => {
                write!(w, "struct{{")?;
                for (i, field) in fields.iter().enumerate() {
                    if i != 0 {
                        write!(w, ", ")?;
                    }
                    field.ir_print(w)?;
                }
                write!(w, "}}")
            }
        }
    }
}

impl<'ctx> IRPrint for Ty<'ctx> {
    #[inline]
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result { self.as_ref().ir_print(w) }
}

impl<'ctx> IRPrint for Instr<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        match &self.kind {
            InstrK::LdInt(n) => write!(w, "ld.int {}", n),
            InstrK::LdFloat(f) => write!(w, "ld.float {}", f),
            InstrK::IAdd => write!(w, "iadd"),
            InstrK::ISub => write!(w, "isub"),
            InstrK::IMul => write!(w, "imul"),
            InstrK::IDiv => write!(w, "idiv"),
            InstrK::FAdd => write!(w, "fadd"),
            InstrK::FSub => write!(w, "fsub"),
            InstrK::FMul => write!(w, "fmul"),
            InstrK::FDiv => write!(w, "fdiv"),
            InstrK::Itof => write!(w, "itof"),
            InstrK::Ftoi => write!(w, "ftoi"),
            InstrK::ICmp(cmp) => match cmp {
                Cmp::Eq => write!(w, "icmp.eq"),
                Cmp::Ne => write!(w, "icmp.ne"),
                Cmp::Lt => write!(w, "icmp.lt"),
                Cmp::Le => write!(w, "icmp.le"),
                Cmp::Gt => write!(w, "icmp.gt"),
                Cmp::Ge => write!(w, "icmp.ge"),
            },
            InstrK::FCmp(cmp) => match cmp {
                Cmp::Eq => write!(w, "fcmp.eq"),
                Cmp::Ne => write!(w, "fcmp.ne"),
                Cmp::Lt => write!(w, "fcmp.lt"),
                Cmp::Le => write!(w, "fcmp.le"),
                Cmp::Gt => write!(w, "fcmp.gt"),
                Cmp::Ge => write!(w, "fcmp.ge"),
            },
            InstrK::CallDirect { func_name } => write!(w, "call \"{}\"", func_name),
            InstrK::LdLocal { idx } => write!(w, "ld.loc #{}", idx),
            InstrK::StLocal { idx } => write!(w, "st.loc #{}", idx),
            InstrK::LdGlobalFunc { func_name } => write!(w, "ld_glob_func \"{}\"", func_name),
            InstrK::CallIndirect => write!(w, "call indirect"),
            InstrK::End => write!(w, "end"),
            InstrK::Bitcast { target } => {
                write!(w, "bitcast to ")?;
                target.ir_print(w)
            }
            InstrK::IfElse { then, r#else } => {
                write!(w, "if then b{}", then.id())?;
                if let Some(else_block) = r#else {
                    write!(w, " else b{}", else_block.id())?
                }
                Ok(())
            }
            InstrK::Read { ty } => {
                write!(w, "read ")?;
                ty.ir_print(w)
            }
            InstrK::Write { ty } => {
                write!(w, "write ")?;
                ty.ir_print(w)
            }
            InstrK::Offset { ty } => {
                write!(w, "offset ")?;
                ty.ir_print(w)
            }
            InstrK::GetFieldPtr { struct_ty, field_idx } => {
                write!(w, "get_field_ptr {} ", field_idx)?;
                struct_ty.ir_print(w)
            }
            InstrK::Discard => write!(w, "discard"),
            InstrK::Return => write!(w, "return"),
            InstrK::Intrinsic(_) => write!(w, "intrinsic ?"), // TODO
        }?;

        if !self.meta.is_empty() {
            write!(w, "  # ")?;
            self.meta.ir_print(w)?;
        }

        writeln!(w)
    }
}

impl IRPrint for BlockId {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "b{}", self.id())
    }
}

impl<'ctx> IRPrint for InstrBlock<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "b{}:", self.idx.id())?;

        if !self.meta.is_empty() {
            write!(w, "  # ")?;
            self.meta.ir_print(w)?;
        }
        writeln!(w)?;

        for instr in &self.body {
            write!(w, "  ")?; // indentation
            instr.ir_print(w)?;
        }

        Ok(())
    }
}

impl<'ctx> IRPrint for Function<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "func \"{}\" ", self.name())?;
        self.ty().ir_print(w)?;
        writeln!(w, " {{")?;

        writeln!(w, "locals:")?;
        for (loc_i, loc_ty) in self.all_locals_ty().iter().enumerate() {
            write!(w, "  #{} ", loc_i)?;
            loc_ty.ir_print(w)?;
            writeln!(w)?;
        }

        // Now, sort all block indexes by number
        // because they are stored in a HashMap and order is not guaranteed
        let mut block_indexes: Vec<BlockId> = self.blocks_iter().map(|b| b.idx).collect();
        block_indexes.sort();

        for block_id in block_indexes {
            self.get_block(block_id).unwrap().ir_print(w)?;
        }

        writeln!(w, "}}")?;
        writeln!(w)
    }
}

impl<'ctx> IRPrint for Module<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        for f in self.functions_iter() {
            f.ir_print(w)?;
        }
        Ok(())
    }
}