use std::{collections::HashMap, fmt::Debug, rc::Rc, sync::Arc};

use derive_more::{From, Into};
use indexmap::IndexSet;

use crate::{
    ast::{Block, Field, Input, Primitive, Script, Variable},
    interpreter::{opcode::Opcode, value::Value},
};

pub type BlockCompiler = Arc<dyn Fn(CompileContext<'_>) + Send + Sync>;

pub struct BlockLibrary {
    blocks: HashMap<&'static str, BlockCompiler>,
    reporters: HashMap<&'static str, BlockCompiler>,
}

impl BlockLibrary {
    pub fn empty() -> Self {
        BlockLibrary {
            blocks: HashMap::new(),
            reporters: HashMap::new(),
        }
    }

    pub fn register_block(
        &mut self,
        name: &'static str,
        block: impl Fn(CompileContext<'_>) + Send + Sync + 'static,
    ) {
        self.blocks.insert(name, Arc::new(block));
    }

    pub fn register_reporter(
        &mut self,
        name: &'static str,
        block: impl Fn(CompileContext<'_>) + Send + Sync + 'static,
    ) {
        self.reporters.insert(name, Arc::new(block));
    }

    pub fn block(&self, opcode: &str) -> Option<BlockCompiler> {
        self.blocks.get(opcode).cloned()
    }

    pub fn reporter(&self, opcode: &str) -> Option<BlockCompiler> {
        self.reporters.get(opcode).cloned()
    }
}

impl Debug for BlockLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("BlockLibrary")
            .field(&format!("{} blocks", self.blocks.len()))
            .finish()
    }
}

impl Default for BlockLibrary {
    fn default() -> Self {
        let mut library = Self::empty();

        library.register_block("data_setvariableto", |ctx| {
            let variable = ctx.block.var_field("VARIABLE");
            let value = &ctx.block.inputs["VALUE"];

            let handle = ctx.compiler.target.var(variable.id());

            ctx.compiler.push_value(value);
            ctx.compiler.write_op(Opcode::SetVar);
            ctx.compiler.write_imm(handle.into());
        });

        library
    }
}

pub struct CompileContext<'a> {
    compiler: &'a mut ScriptCompiler,
    block: &'a Block,
}

#[derive(Debug)]
pub struct ScriptCompiler {
    pub target: TargetContext,
    pub library: Arc<BlockLibrary>,
    pub data: Vec<u32>,
}

impl ScriptCompiler {
    pub fn new(target: TargetContext, library: Arc<BlockLibrary>) -> Self {
        Self {
            target,
            library,
            data: vec![],
        }
    }

    pub fn compile(&mut self, script: &Script) {
        for block in &script.blocks {
            self.compile_block(block);
        }
    }

    pub fn set_var(&mut self, variable: Variable, value: Input) {
        let handle = self.target.var(variable.id());

        self.push_value(&value);
        self.write_op(Opcode::SetVar);
        self.write_imm(handle.into());
    }

    fn context<'a>(&'a mut self, block: &'a Block) -> CompileContext<'a> {
        CompileContext {
            compiler: self,
            block,
        }
    }

    pub fn compile_block(&mut self, block: &Block) {
        let Some(block_impl) = self.library.block(&block.opcode) else {
            unimplemented!("block opcode {}", block.opcode)
        };

        block_impl(self.context(block));
    }

    pub fn push_value(&mut self, input: &Input) {
        let [block] = &input.blocks[..] else {
            panic!("Expected single value, found substack");
        };

        if let Some(primitive) = block.try_as_primitive() {
            self.push_primitive(primitive);
            return;
        }

        let Some(block_impl) = self.library.reporter(&block.opcode) else {
            unimplemented!("reporter block opcode {}", block.opcode)
        };

        block_impl(self.context(block));
    }

    pub fn push_primitive(&mut self, primitive: Primitive) {
        match primitive {
            Primitive::Text(string) => {
                let handle = self.target.text(string);
                self.push_constant(handle);
            }
            Primitive::Number(num) | Primitive::PositiveNumber(num) | Primitive::Angle(num) => {
                self.push_f64(num)
            }
            Primitive::Integer(num) => self.push_f64(num as f64),
            Primitive::WholeNumber(num) => self.push_f64(num as f64),
            Primitive::Variable(var) => {
                let handle = self.target.var(var.id());
                self.write_op(Opcode::PushVar);
                self.write_imm(handle.into());
            }
            Primitive::Event(_) => {
                panic!("events cannot be pushed to the stack");
            }
        }
    }

    pub fn push_constant(&mut self, handle: ConstantHandle) {
        self.write_op(Opcode::PushConstant);
        self.write_imm(handle.into());
    }

    pub fn push_f64(&mut self, num: f64) {
        self.write_op(Opcode::PushNumber);
        self.write_u64(num.to_bits());
    }

    pub fn write_op(&mut self, opcode: Opcode) {
        self.data.push(opcode as _);
    }

    pub fn write_imm(&mut self, immediate: u32) {
        self.data.push(immediate);
    }

    pub fn write_u64(&mut self, num: u64) {
        let parts: [u32; 2] = bytemuck::cast(num.to_le_bytes());
        for part in parts {
            self.write_imm(part);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TargetContext {
    variables: IndexSet<Rc<str>>,
    text_consts: IndexSet<Rc<str>>,
}

impl TargetContext {
    pub fn var(&mut self, id: Rc<str>) -> VarHandle {
        let (idx, _exists) = self.variables.insert_full(id);
        VarHandle::from(idx as u32)
    }

    pub fn text(&mut self, value: Rc<str>) -> ConstantHandle {
        let (idx, _exists) = self.text_consts.insert_full(value);
        ConstantHandle::from(idx as u32)
    }
}

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct ConstantHandle(u32);

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct VarHandle(u32);
