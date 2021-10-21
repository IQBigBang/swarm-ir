use crate::{instr::{BlockId, BlockTag, Cmp, Function, Instr, InstrBlock, InstrK}, module::{ExternFunction, FuncDef, Functional, Global, Module}, numerics::BitWidthSign, ty::{Ty, Type}};

pub trait IRPrint {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result;
}

impl<'ctx> IRPrint for Type<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        match self {
            Type::Int32 => write!(w, "int32"),
            Type::UInt32 => write!(w, "uint32"),
            Type::Int16 => write!(w, "int16"),
            Type::UInt16 => write!(w, "uint16"),
            Type::Int8 => write!(w, "int8"),
            Type::UInt8 => write!(w, "uint8"),
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
            InstrK::LdInt(n, ty) => {
                write!(w, "ld.")?;
                ty.ir_print(w)?;
                write!(w, " {}", n)
            }
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
            InstrK::Ftoi { int_ty } => {
                write!(w, "ftoi to ")?;
                int_ty.ir_print(w)
            },
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
            InstrK::IConv { target } => {
                write!(w, "iconv to ")?;
                target.ir_print(w)
            }
            InstrK::CallDirect { func_name } => write!(w, "call \"{}\"", func_name),
            InstrK::LdLocal { idx } => write!(w, "ld.loc #{}", idx),
            InstrK::StLocal { idx } => write!(w, "st.loc #{}", idx),
            InstrK::LdGlobalFunc { func_name } => write!(w, "ld_glob_func \"{}\"", func_name),
            InstrK::CallIndirect => write!(w, "call indirect"),
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
            InstrK::MemorySize => write!(w, "memory.size"),
            InstrK::MemoryGrow => write!(w, "memory.grow"),
            InstrK::LdGlobal(name) => write!(w, "ld.global \"{}\"", name),
            InstrK::StGlobal(name) => write!(w, "st.global \"{}\"", name),
            InstrK::Fail => write!(w, "fail"),
            InstrK::Loop(body) => write!(w, "loop b{}", body.id()),
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
        write!(w, "b{}: ", self.idx.id())?;
        self.full_type().ir_print(w)?;
        write!(w, " tag={}", match self.tag {
            BlockTag::Undefined => "undefined",
            BlockTag::Main => "main",
            BlockTag::IfElse => "if_else",
            BlockTag::Loop => "loop",
        })?;

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

impl<'ctx> IRPrint for Global<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "global \"{}\" = ", self.name)?;
        if self.is_int() {
            write!(w, "int32 {}", self.get_int_value())?;
        } else {
            write!(w, "float32 {}", self.get_float_value())?;
        }
        writeln!(w)
    }
}

impl<'ctx> IRPrint for Module<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        for g in self.globals_iter() {
            g.ir_print(w)?;
        }
        writeln!(w)?;
        for f in self.functions_iter() {
            f.ir_print(w)?;
        }
        Ok(())
    }
}

impl IRPrint for BitWidthSign {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        match self {
            BitWidthSign::S32 => write!(w, "s32"),
            BitWidthSign::U32 => write!(w, "u32"),
            BitWidthSign::S16 => write!(w, "s16"),
            BitWidthSign::U16 => write!(w, "u16"),
            BitWidthSign::S8 => write!(w, "s8"),
            BitWidthSign::U8 => write!(w, "u8"),
        }
    }
}

impl<'ctx> IRPrint for ExternFunction<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "extern func \"{}\" ", self.name())?;
        self.ty().ir_print(w)?;
        writeln!(w, ";")?;
        writeln!(w)
    }
}

impl<'ctx> IRPrint for FuncDef<'ctx> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        match self {
            FuncDef::Local(f) => f.ir_print(w),
            FuncDef::Extern(f) => f.ir_print(w), 
        }
    }
}