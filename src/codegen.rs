use std::{collections::HashMap, fmt::Debug, mem, rc::Rc, sync::Arc};

use bon::bon;
use derive_more::{From, Into};
use indexmap::{IndexMap, IndexSet};

use crate::{
    ast::{Block, Field, Input, Primitive, Script, Variable, VariableRef},
    interpreter::{self, RuntimeContext, opcode::Opcode, value::Value},
};

pub type BlockCompileLogic = Arc<dyn Fn(CompileContext<'_>) + Send + Sync>;
pub type BlockRuntimeLogic = Box<dyn FnMut(RuntimeContext<'_>) + Send + Sync>;

#[derive(Clone)]
pub struct KnownBlock {
    name: Arc<str>,
    compile_logic: Option<BlockCompileLogic>,
    id: u32,
}

impl KnownBlock {
    pub fn compile(&self, compiler: &mut ScriptCompiler, block: &Block) {
        if let Some(compile_logic) = &self.compile_logic {
            compile_logic(CompileContext {
                compiler,
                block,
                id: self.id,
            });
        } else {
            compiler.compile_runtime_only(block, self.id);
        }
    }

    pub fn name(&self) -> Arc<str> {
        self.name.clone()
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

struct LibraryStorage {
    compile_logic: Option<BlockCompileLogic>,
    runtime_logic: Option<BlockRuntimeLogic>,
    is_reporter: bool,
}

pub struct BlockLibrary {
    blocks: IndexMap<Arc<str>, LibraryStorage>,
}

#[bon]
impl BlockLibrary {
    pub fn empty() -> Self {
        BlockLibrary {
            blocks: IndexMap::new(),
        }
    }

    #[builder(finish_fn = finish)]
    pub fn register_block(
        &mut self,
        #[builder(start_fn, into)] name: Arc<str>,
        #[builder(with = |c: impl Fn(CompileContext<'_>) + Send + Sync + 'static| Arc::new(c))]
        compile_logic: Option<BlockCompileLogic>,
        #[builder(with = |c: impl FnMut(RuntimeContext<'_>) + Send + Sync + 'static| Box::new(c))]
        runtime_logic: Option<BlockRuntimeLogic>,
    ) -> u32 {
        self.register_impl(name, compile_logic, runtime_logic, false)
    }

    #[builder(finish_fn = finish)]
    pub fn register_reporter(
        &mut self,

        #[builder(start_fn, into)] name: Arc<str>,
        #[builder(with = |c: impl Fn(CompileContext<'_>) + Send + Sync + 'static| Arc::new(c))]
        compile_logic: Option<BlockCompileLogic>,
        #[builder(with = |c: impl FnMut(RuntimeContext<'_>) + Send + Sync + 'static| Box::new(c))]
        runtime_logic: Option<BlockRuntimeLogic>,
    ) -> u32 {
        self.register_impl(name, compile_logic, runtime_logic, true)
    }

    fn register_impl(
        &mut self,
        name: Arc<str>,
        compile_logic: Option<BlockCompileLogic>,
        runtime_logic: Option<BlockRuntimeLogic>,
        is_reporter: bool,
    ) -> u32 {
        let (idx, _) = self.blocks.insert_full(
            name,
            LibraryStorage {
                compile_logic,
                runtime_logic,
                is_reporter,
            },
        );
        idx as u32
    }

    pub fn block(&self, opcode: &str) -> Option<KnownBlock> {
        self.blocks
            .get_full(opcode)
            .filter(|(_, _, storage)| !storage.is_reporter)
            .map(|(idx, name, storage)| KnownBlock {
                name: name.clone(),
                compile_logic: storage.compile_logic.clone(),
                id: idx as u32,
            })
    }

    pub fn reporter(&self, opcode: &str) -> Option<KnownBlock> {
        self.blocks
            .get_full(opcode)
            .filter(|(_, _, storage)| storage.is_reporter)
            .map(|(idx, name, storage)| KnownBlock {
                name: name.clone(),
                compile_logic: storage.compile_logic.clone(),
                id: idx as u32,
            })
    }

    pub fn into_runtime_callbacks(self) -> Vec<Option<BlockRuntimeLogic>> {
        self.blocks
            .into_values()
            .map(|storage| storage.runtime_logic)
            .collect()
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

        library
            .register_block("looks_say")
            .runtime_logic(|mut ctx| {
                let param = ctx.stack_mut().pop().unwrap();
                println!("{}", ctx.program().dbg_string(&param));
            })
            .finish();

        library
            .register_block("data_setvariableto")
            .compile_logic(|ctx| {
                let variable = ctx.block.var_field("VARIABLE");
                let value = &ctx.block.inputs["VALUE"];

                let handle = ctx.compiler.target.var(variable);

                ctx.compiler.push_value(value);
                ctx.compiler.write_op(Opcode::SetVar);
                ctx.compiler.write_imm(handle.into());
            })
            .finish();

        library
    }
}

pub struct CompileContext<'a> {
    pub compiler: &'a mut ScriptCompiler,
    pub block: &'a Block,
    pub id: u32,
}

#[derive(Debug)]
pub struct ScriptCompiler {
    pub target: Arc<TargetContext>,
    pub library: Arc<BlockLibrary>,
    pub data: Vec<u32>,
}

impl ScriptCompiler {
    pub fn new(target: Arc<TargetContext>, library: Arc<BlockLibrary>) -> Self {
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
        self.write_op(Opcode::Return);
    }

    pub fn set_var(&mut self, variable: VariableRef, value: Input) {
        let handle = self.target.var(variable);

        self.push_value(&value);
        self.write_op(Opcode::SetVar);
        self.write_imm(handle.into());
    }

    pub fn compile_block(&mut self, block: &Block) {
        let Some(handler) = self.library.block(&block.opcode) else {
            unimplemented!("block opcode {}", block.opcode)
        };

        handler.compile(self, block);
    }

    pub fn push_value(&mut self, input: &Input) {
        let [block] = &input.blocks[..] else {
            panic!("Expected single value, found substack");
        };

        if let Some(primitive) = block.try_as_primitive() {
            self.push_primitive(primitive);
            return;
        }

        let Some(handler) = self.library.reporter(&block.opcode) else {
            unimplemented!("reporter opcode {}", block.opcode)
        };

        handler.compile(self, block);
    }

    fn compile_runtime_only(&mut self, block: &Block, runtime_id: u32) {
        // Assume runtime-only implementation. Fields aren't be represented
        // on the stack, so we disallow them.
        if !block.fields.is_empty() {
            panic!("missing compile-time implementation for {:?}", block.opcode);
        }

        let mut inputs: Vec<_> = block.inputs.iter().collect();
        inputs.sort_unstable_by_key(|&(k, _)| &**k);

        for (_key, input) in inputs {
            self.push_value(input);
        }

        self.write_op(Opcode::CallBuiltin);
        self.write_imm(runtime_id);
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
                let handle = self.target.var(var);
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

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub variables: IndexMap<Arc<str>, Variable>,
    pub text_consts: IndexSet<Arc<str>>,
}

impl ProjectContext {
    pub fn new(variables: impl IntoIterator<Item = Variable>, text_consts: IndexSet<Arc<str>>) -> Self {
        Self {
            variables: IndexMap::from_iter(variables.into_iter().map(|var| (var.id(), var))),
            text_consts: IndexSet::from_iter(text_consts),
        }
    }

    pub fn text(&self, value: Arc<str>) -> ConstantHandle {
        let idx = self.text_consts.get_index_of(&value).expect("Text missing from context pool");
        ConstantHandle::from(idx as u32)
    }

    pub fn take_constants(&mut self) -> Box<[Value]> {
        self.text_consts.drain(..).map(Value::String).collect()
    }
}

#[derive(Debug, Clone)]
pub struct TargetContext {
    pub project: Arc<ProjectContext>,
    pub variables: IndexMap<Arc<str>, Variable>,
}

impl TargetContext {
    pub fn new(
        project_ctx: Arc<ProjectContext>,
        sprite_vars: impl IntoIterator<Item = Variable>,
    ) -> Self {
        let mut vars_lookup_map = project_ctx.variables.clone();
        vars_lookup_map.extend(sprite_vars.into_iter().map(|var| (var.id(), var)));

        Self {
            variables: vars_lookup_map,
            project: project_ctx,
        }
    }

    pub fn var(&self, var: VariableRef) -> VarHandle {
        let idx = self
            .variables
            .get_index_of(&var.id())
            .expect("unknown variable");

        VarHandle::from(idx as u32)
    }

    pub fn text(&self, value: Arc<str>) -> ConstantHandle {
        self.project.text(value)
    }

    pub fn take_vars(&mut self) -> Vec<Variable> {
        self.variables.drain(..).map(|(_id, state)| state).collect()
    }
}

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct ConstantHandle(u32);

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct VarHandle(u32);
