use std::{collections::HashMap, iter::Peekable};

use logos::{Logos, SpannedIter};

use crate::{instr::{BlockId, BlockTag, Cmp, Function, Instr, InstrBlock, InstrK}, module::Module, ty::{Ty, Type}};

#[derive(Logos, PartialEq, Debug)]
pub enum IrToken {
    #[token("int32")]
    Int32,
    #[token("float32")]
    Float32,
    #[token("uint32")]
    UInt32,
    #[token("int16")]
    Int16,
    #[token("uint16")]
    UInt16,
    #[token("int8")]
    Int8,
    #[token("uint8")]
    UInt8,
    #[token("ptr")]
    Ptr,
    #[token("struct")]
    Struct,
    #[token("func")]
    Func,
    #[regex(r#""([^"])*""#)]
    String,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token(",")]
    Comma,
    #[token("->")]
    Arrow,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token("locals")]
    Locals,
    #[token("#")]
    Hash,
    #[regex("[0-9]+")]
    Int,
    #[regex("[0-9]+'.'[0-9]+")]
    Float,
    #[regex("[a-zA-Z_][a-zA-Z0-9._-]*")] // identifiers may contain dots
    Identifier,
    #[token("=")]
    Equals,
    #[regex(r"[ \n\r\t\f]+", logos::skip)]
    #[error]
    Error,
}

pub struct IRParser<'a, 'ctx> {
    module: &'a mut Module<'ctx>,
    source: &'a str,
    lex: Peekable<SpannedIter<'a, IrToken>>
}

impl<'a, 'ctx> IRParser<'a, 'ctx> {
    pub fn new(module: &'a mut Module<'ctx>, source: &'a str) -> Self {
        Self {
            module,
            source,
            lex: IrToken::lexer(source).spanned().peekable()
        }
    }

    fn expect(&mut self, expected: IrToken) -> Result<&'a str, IrParseError> {
        match self.lex.next() {
            None => Err(IrParseError::UnexpectedEof),
            Some((t, span)) => if t == expected {
                Ok(&self.source[span])
            } else {
                Err(IrParseError::UnexpectedToken { expected, got: t })
            }
        }
    }

    fn peek(&mut self, expected: IrToken) -> bool {
        match self.lex.peek() {
            None => false,
            Some((t, _)) => t == &expected
        }
    }

    fn peek_str(&mut self, expected: IrToken) -> Option<&str> {
        match self.lex.peek() {
            None => None,
            Some((t, span)) => if t == &expected {
                Some(&self.source[span.clone()])
            } else {
                None
            }
        }
    }

    fn next(&mut self) -> Option<(IrToken, &'a str)> {
        self.lex.next().map(|(t, span)| (t, &self.source[span]))
    }

    fn parse_block_id(&mut self) -> Result<BlockId, IrParseError> {
        // the block str is "b{number}"
        let block_str = self.expect(IrToken::Identifier)?;
        if !block_str.starts_with('b') {
            return Err(IrParseError::MalformedIdentifier { got: block_str.to_owned() });
        }
        let block_id: usize = block_str[1..].parse().unwrap();
        Ok(BlockId::from(block_id))
    }

    fn parse_instr(&mut self) -> Result<Instr<'ctx>, IrParseError> {
        let i = match self.expect(IrToken::Identifier)? {
            "ld.int32" => {
                let n = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdInt(n, self.module.int32t()))
            }
            "ld.uint32" => {
                let n = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdInt(n, self.module.uint32t()))
            }
            "ld.int16" => {
                let n = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdInt(n, self.module.int16t()))
            }
            "ld.uint16" => {
                let n = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdInt(n, self.module.uint16t()))
            }
            "ld.int8" => {
                let n = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdInt(n, self.module.int8t()))
            }
            "ld.uint8" => {
                let n = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdInt(n, self.module.uint8t()))
            }
            "ld.float" => {
                let f = self.expect(IrToken::Float)?.parse().unwrap();
                Instr::new(InstrK::LdFloat(f))
            }
            "iadd" => Instr::new(InstrK::IAdd),
            "isub" => Instr::new(InstrK::ISub),
            "imul" => Instr::new(InstrK::IMul),
            "idiv" => Instr::new(InstrK::IDiv),
            "fadd" => Instr::new(InstrK::FAdd),
            "fsub" => Instr::new(InstrK::FSub),
            "fmul" => Instr::new(InstrK::FMul),
            "fdiv" => Instr::new(InstrK::FDiv),
            "itof" => Instr::new(InstrK::Itof),
            "ftoi" => {
                let t = self.expect(IrToken::Identifier)?;
                if t != "to" { return Err(IrParseError::MalformedIdentifier { got: t.to_owned() }) }
                let int_ty = self.parse_type()?;
                Instr::new(InstrK::Ftoi { int_ty })
            },
            "icmp.eq" => Instr::new(InstrK::ICmp(Cmp::Eq)),
            "icmp.ne" => Instr::new(InstrK::ICmp(Cmp::Ne)),
            "icmp.lt" => Instr::new(InstrK::ICmp(Cmp::Lt)),
            "icmp.le" => Instr::new(InstrK::ICmp(Cmp::Le)),
            "icmp.gt" => Instr::new(InstrK::ICmp(Cmp::Gt)),
            "icmp.ge" => Instr::new(InstrK::ICmp(Cmp::Ge)),
            "fcmp.eq" => Instr::new(InstrK::FCmp(Cmp::Eq)),
            "fcmp.ne" => Instr::new(InstrK::FCmp(Cmp::Ne)),
            "fcmp.lt" => Instr::new(InstrK::FCmp(Cmp::Lt)),
            "fcmp.le" => Instr::new(InstrK::FCmp(Cmp::Le)),
            "fcmp.gt" => Instr::new(InstrK::FCmp(Cmp::Gt)),
            "fcmp.ge" => Instr::new(InstrK::FCmp(Cmp::Ge)),
            "iconv" => {
                let t = self.expect(IrToken::Identifier)?;
                if t != "to" { return Err(IrParseError::MalformedIdentifier { got: t.to_owned() }) }
                let target = self.parse_type()?;
                Instr::new(InstrK::IConv { target })
            },
            "call" => {
                if self.peek(IrToken::Identifier) {
                    // call indirect
                    let t = self.expect(IrToken::Identifier)?;
                    if t != "indirect" { return Err(IrParseError::MalformedIdentifier  { got: t.to_owned() }) }
                    Instr::new(InstrK::CallIndirect)
                } else {
                    let func_name = self.expect(IrToken::String)?.strip('"').to_owned();
                    Instr::new(InstrK::CallDirect { func_name })
                }
            },
            "ld.loc" => {
                self.expect(IrToken::Hash)?;
                let idx = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::LdLocal { idx })
            }
            "st.loc" => {
                self.expect(IrToken::Hash)?;
                let idx = self.expect(IrToken::Int)?.parse().unwrap();
                Instr::new(InstrK::StLocal { idx })
            }
            "ld_glob_func" => {
                let func_name = self.expect(IrToken::String)?.strip('"').to_owned();
                Instr::new(InstrK::LdGlobalFunc { func_name })
            },
            "bitcast" => {
                let t = self.expect(IrToken::Identifier)?;
                if t != "to" { return Err(IrParseError::MalformedIdentifier { got: t.to_owned() }) }
                let target = self.parse_type()?;
                Instr::new(InstrK::Bitcast { target })
            }
            "if" => {
                let t = self.expect(IrToken::Identifier)?;
                if t != "then" { return Err(IrParseError::MalformedIdentifier { got: t.to_owned() }) }
                let then = self.parse_block_id()?;
                if self.peek_str(IrToken::Identifier) == Some("else") {
                    self.next(); // "else"
                    let else_block = self.parse_block_id()?;
                    Instr::new(InstrK::IfElse { then, r#else: Some(else_block) })
                } else {
                    Instr::new(InstrK::IfElse { then, r#else: None })
                }
            }
            "read" => {
                let ty = self.parse_type()?;
                Instr::new(InstrK::Read { ty })
            }
            "write" => {
                let ty = self.parse_type()?;
                Instr::new(InstrK::Write { ty })
            }
            "offset" => {
                let ty = self.parse_type()?;
                Instr::new(InstrK::Offset { ty })
            }
            "get_field_ptr" => {
                let field_idx = self.expect(IrToken::Int)?.parse().unwrap();
                let struct_ty = self.parse_type()?;
                Instr::new(InstrK::GetFieldPtr { struct_ty, field_idx })
            }
            "discard" => Instr::new(InstrK::Discard),
            "return" => Instr::new(InstrK::Return),
            "memory.size" => Instr::new(InstrK::MemorySize),
            "memory.grow" => Instr::new(InstrK::MemoryGrow),
            "ld.global" => {
                let name = self.expect(IrToken::String)?.strip('"').to_owned();
                Instr::new(InstrK::LdGlobal(name))
            }
            "st.global" => {
                let name = self.expect(IrToken::String)?.strip('"').to_owned();
                Instr::new(InstrK::StGlobal(name))
            }
            "fail" => Instr::new(InstrK::Fail),
            _ => return Err(IrParseError::InvalidInstructionName)
        };
        Ok(i)
    }

    fn parse_block(&mut self) -> Result<InstrBlock<'ctx>, IrParseError> {
        let id = self.parse_block_id()?;
        self.expect(IrToken::Colon)?;

        let block_ty = self.parse_type()?;

        // parse the "tag=smth"
        let t = self.expect(IrToken::Identifier)?;
        if t != "tag" { return Err(IrParseError::MalformedIdentifier { got: t.to_owned() }) }
        self.expect(IrToken::Equals)?;
        let block_tag = match self.expect(IrToken::Identifier)? {
            "undefined" => BlockTag::Undefined,
            "main" => BlockTag::Main,
            "if_else" => BlockTag::IfElse,
            "loop" => BlockTag::Loop,
            other => return Err(IrParseError::MalformedIdentifier { got: other.to_owned() })
        };

        let mut block = InstrBlock::new(id, block_ty, block_tag);

        // then parse the instructions
        fn is_block_id(s: &str) -> bool {
            s.chars().nth(0) == Some('b')
            && s.chars().nth(1).map(|c| c.is_digit(10)).unwrap_or(false)
        }
    
        // FIXME a dirty hack, a block doesn't have a formal ending
        // but it's usually (always?) followed by either a new block or a '}'
        loop {
            if self.peek_str(IrToken::Identifier).map(is_block_id).unwrap_or(false) { break; }
            if self.peek(IrToken::RBrace) { break; }
            if self.lex.peek().is_none() { break; }

            block.body.push(self.parse_instr()?);
        }
        Ok(block)
    }

    pub fn parse_function(&mut self) -> Result<Function<'ctx>, IrParseError> {
        self.expect(IrToken::Func)?;
        let func_name = self.expect(IrToken::String)?.strip('"').to_owned();
        let func_ty = self.parse_type()?;

        self.expect(IrToken::LBrace)?;
        self.expect(IrToken::Locals)?;
        self.expect(IrToken::Colon)?;
    
        let mut locals = vec![];
        while self.peek(IrToken::Hash) {
            self.next(); // '#'
            let local_id: usize = self.expect(IrToken::Int)?.parse().unwrap();
            let local_ty = self.parse_type()?;
            locals.insert(local_id, local_ty);
        }

        let mut blocks = HashMap::new();
        while !self.peek(IrToken::RBrace) { // closing brace of the function
            let block = self.parse_block()?;
            blocks.insert(block.idx, block);
        }
        self.next(); // '}'

        Ok(Function::new(func_name, func_ty, blocks, locals))
    }

    fn parse_type(&mut self) -> Result<Ty<'ctx>, IrParseError> {
        if self.peek(IrToken::Int32) {
            self.next();
            Ok(self.module.int32t())
        } else if self.peek(IrToken::UInt32) {
            self.next();
            Ok(self.module.uint32t())
        } else if self.peek(IrToken::Int16) {
            self.next();
            Ok(self.module.int16t())
        } else if self.peek(IrToken::UInt16) {
            self.next();
            Ok(self.module.uint16t())
        } else if self.peek(IrToken::Int8) {
            self.next();
            Ok(self.module.int8t())
        } else if self.peek(IrToken::UInt8) {
            self.next();
            Ok(self.module.uint8t())
        } else if self.peek(IrToken::Float32) {
            self.next();
            Ok(self.module.float32t())
        } else if self.peek(IrToken::Ptr) {
            self.next();
            Ok(self.module.ptr_t())
        } else if self.peek(IrToken::LParen) {
            // a function type
            self.next(); // '('
            let mut args = vec![];
            while !self.peek(IrToken::RParen) {
                args.push(self.parse_type()?);
                if self.peek(IrToken::Comma) { self.next(); }
            }
            self.next(); // ')'
            self.expect(IrToken::Arrow)?;
            // now it's a bit more complicated, because if rets=[], then there will be '()'
            // but if it's a single return value, there's no parentheses!
            let mut rets = vec![];
            if self.peek(IrToken::LParen) {
                self.next(); // '('
                while !self.peek(IrToken::RParen) {
                    rets.push(self.parse_type()?);
                    if self.peek(IrToken::Comma) { self.next(); }
                }
                self.next(); // ')'
            } else {
                // no parens => a single return value
                rets.push(self.parse_type()?);
            }
            
            Ok(self.module.intern_type(Type::Func { args, ret: rets }))
        } else if self.peek(IrToken::Struct) {
            // a struct type
            self.next(); // 'struct'
            self.expect(IrToken::LBrace)?;
            let mut fields = vec![];
            while !self.peek(IrToken::RBrace) {
                fields.push(self.parse_type()?);
                if self.peek(IrToken::Comma) { self.next(); }
            }

            Ok(self.module.intern_type(Type::Struct { fields }))
        } else { 
            Err(IrParseError::GeneralUnexpectedToken) 
        }
    }
}

// A helper method on &str
// used to strip the start and end double quotes off a parsed string
// "\"name\"".strip('"') == "name"
trait StrHelper {
    fn strip(&self, c: char) -> &str;
}
impl<T: AsRef<str>> StrHelper for T {
    fn strip(&self, c: char) -> &str {
        let s = self.as_ref();
        let strip_first = s.chars().nth(0) == Some(c);
        let strip_last = s.chars().nth_back(0) == Some(c);
        &s[(if strip_first {1} else {0}) .. (s.len() - if strip_last {1} else {0})]
    }
}

#[derive(Debug)]
pub enum IrParseError {
    UnexpectedEof,
    UnexpectedToken { expected: IrToken, got: IrToken },
    GeneralUnexpectedToken,
    MalformedIdentifier { got: String },
    InvalidInstructionName
}