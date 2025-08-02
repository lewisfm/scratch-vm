use std::{cell::RefCell, cmp::Ordering, collections::HashMap, fmt::Debug, mem, rc::Rc, sync::Arc, u32};

use bon::bon;
use derive_more::{From, Into};
use indexmap::{IndexMap, IndexSet};

use crate::{
    ast::{Block, Field, Input, Primitive, Script, Variable, VariableRef},
    blocks::{BlockCompileLogic, BlockTypeLibrary},
    interpreter::{self, opcode::Opcode, value::{Local, Value}, RuntimeContext},
};

#[derive(Clone)]
pub struct BlockType {
    pub(crate) opcode: Arc<str>,
    pub(crate) compile_logic: Option<Arc<BlockCompileLogic>>,
    pub(crate) inputs_order: Vec<Arc<str>>,
    pub(crate) id: u32,
    pub(crate) is_reporter: bool,
}

impl BlockType {
    pub fn compile(&self, compiler: &mut ScriptCompiler, block: &Block) {
        if let Some(compile_logic) = &self.compile_logic {
            compile_logic(CompileContext {
                compiler,
                block,
                id: self.id,
            });
        } else {
            compiler.compile_runtime_only(block, self.id, &self.inputs_order);
        }
    }

    pub fn name(&self) -> Arc<str> {
        self.opcode.clone()
    }

    pub fn id(&self) -> u32 {
        self.id
    }
}

pub struct CompileContext<'a> {
    pub compiler: &'a mut ScriptCompiler,
    pub block: &'a Block,
    pub id: u32,
}

impl CompileContext<'_> {
    pub fn build_call_self(&mut self) {
        self.compiler.write_op(Opcode::CallBuiltin);
        self.compiler.write_imm(self.id);
    }
}

#[derive(Debug)]
pub struct ScriptCompiler {
    pub target: Arc<TargetCodegenContext>,
    pub block_library: Arc<BlockTypeLibrary>,
    pub data: Vec<u32>,
    pub suppress_yields: bool,
    num_proc_params: usize,
    locals: Vec<Option<()>>,
}

impl ScriptCompiler {
    pub fn new(
        target: Arc<TargetCodegenContext>,
        blocks: Arc<BlockTypeLibrary>,
        suppress_yields: bool,
        num_proc_params: usize,
    ) -> Self {
        Self {
            target,
            block_library: blocks,
            data: vec![],
            suppress_yields,
            num_proc_params,
            locals: vec![None; num_proc_params],
        }
    }

    pub fn compile(&mut self, script: &Script) {
        self.compile_substack(&script.blocks);
        self.write_op(Opcode::Return);
    }

    pub fn compile_substack(&mut self, substack: &[Block]) {
        for block in substack {
            self.compile_block(block);
        }

        if substack.is_empty() {
            self.build_yield();
        }
}

    pub fn compile_block(&mut self, block: &Block) {
        let Some(handler) = self.block_library.block(&block.opcode) else {
            unimplemented!("block opcode {}", block.opcode)
        };

        handler.compile(self, block);
    }

    /// Claims a local ID that isn't in use. It should be returned to the compiler
    /// when it's no longer needed so another block can use it.
    pub fn claim_local(&mut self) -> LocalHandle {
        // Look for locals that have been freed so we can reuse them
        for idx in 0..self.locals.len() {
            if self.locals[idx].take().is_some() {
                return LocalHandle(idx as u32);
            }
        }

        // Allocate a new local for this script
        let local = LocalHandle(self.locals.len() as u32);
        self.locals.push(None);
        local
    }

    pub fn release_local(&mut self, handle: LocalHandle) {
        self.locals[handle.0 as usize] = Some(());
    }

    fn compile_runtime_only(&mut self, block: &Block, runtime_id: u32, inputs_order: &[Arc<str>]) {
        // Assume runtime-only implementation. Fields aren't be represented
        // on the stack, so we disallow them.
        if !block.fields.is_empty() {
            panic!("missing compile-time implementation for {:?}", block.opcode);
        }

        let mut inputs: Vec<(&Arc<str>, &Input)>;
        if inputs_order.is_empty() {
            inputs = block.inputs.iter().collect();
            inputs.sort_unstable_by_key(|&(k, _)| &**k);
        } else {
            inputs = Vec::with_capacity(block.inputs.len());
            for key in inputs_order {
                inputs.push((key, &block.inputs[key]));
            }
        }

        for (_key, input) in inputs {
            self.build_push(input);
        }

        self.write_op(Opcode::CallBuiltin);
        self.write_imm(runtime_id);
    }

    pub fn label_here(&self) -> ConcreteLabel {
        ConcreteLabel(self.data.len())
    }

    pub fn commit_placeholder(&mut self, label: PlaceholderLabel) {
        label.finalize_here(self);
    }

    pub fn build_push(&mut self, input: impl StackRepresentable) {
        input.build_push_to_stack(self);
    }

    pub fn build_yield(&mut self) {
        if !self.suppress_yields {
            self.write_op(Opcode::Yield);
        }
    }

    pub fn build_jump(&mut self, destination: impl Label) {
        self.write_op(Opcode::Jump);
        destination.write(self);
    }

    pub fn build_jump_if(&mut self, condition: bool, destination: &impl Label) {
        if condition {
            self.write_op(Opcode::JumpIfTrue);
        } else {
            self.write_op(Opcode::JumpIfFalse);
        }
        destination.write(self);
    }

    pub fn build_set_var(&mut self, variable: VariableRef, value: impl StackRepresentable) {
        let handle = self.target.var(variable);

        self.build_push(value);
        self.write_op(Opcode::SetVar);
        self.write_imm(handle.into());
    }

    pub fn build_change_var(&mut self, variable: VariableRef, value: impl StackRepresentable) {
        let handle = self.target.var(variable);

        self.build_push(value);
        self.write_op(Opcode::ChangeVar);
        self.write_imm(handle.into());
    }

    pub fn build_set_local(&mut self, local_handle: LocalHandle, value: impl StackRepresentable) {
        self.build_push(value);
        self.write_op(Opcode::SetLocal);
        self.write_imm(local_handle.into());
    }

    pub fn build_cmp(&mut self, left: impl StackRepresentable, cmp: Ordering, right: impl StackRepresentable) {
        self.build_push(left);
        self.build_push(right);
        self.write_op(match cmp {
            Ordering::Equal => unimplemented!(),
            Ordering::Greater => Opcode::GreaterThan,
            Ordering::Less => unimplemented!(),
        });
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

    pub fn get_locals(&self) -> Box<[Local]> {
        self.locals
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                let name = if let Some(idx) = idx.checked_sub(self.num_proc_params) {
                    format!("Auto-generated #{idx}")
                } else {
                    format!("Procedure param #{idx}")
                };

                Local::new(Some(name.into()))
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    pub variables: IndexMap<Arc<str>, Variable>,
    pub text_consts: Arc<IndexSet<Arc<str>>>,
}

impl ProjectContext {
    pub fn new(
        variables: impl IntoIterator<Item = Variable>,
        text_consts: Arc<IndexSet<Arc<str>>>,
    ) -> Self {
        Self {
            variables: IndexMap::from_iter(variables.into_iter().map(|var| (var.id(), var))),
            text_consts,
        }
    }

    pub fn text(&self, value: Arc<str>) -> ConstantHandle {
        let idx = self
            .text_consts
            .get_index_of(&value)
            .expect("Text missing from context pool");
        ConstantHandle::from(idx as u32)
    }
}

#[derive(Debug, Clone)]
pub struct TargetCodegenContext {
    pub project: Arc<ProjectContext>,
    pub variables: IndexMap<Arc<str>, Variable>,
}

impl TargetCodegenContext {
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

pub trait StackRepresentable {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler);
}

impl StackRepresentable for &Input {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler) {
        let [block] = &self.blocks[..] else {
            panic!("Expected single value, found substack");
        };

        if let Some(primitive) = block.try_as_primitive() {
            compiler.build_push(primitive);
            return;
        }

        let Some(handler) = compiler.block_library.reporter(&block.opcode) else {
            unimplemented!("reporter opcode {}", block.opcode)
        };

        handler.compile(compiler, block);
    }
}

impl StackRepresentable for Primitive {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler) {
        match self {
            Primitive::Text(string) => {
                compiler.build_push(compiler.target.text(string));
            }
            Primitive::Number(num) | Primitive::PositiveNumber(num) | Primitive::Angle(num) => {
                compiler.build_push(num)
            }
            Primitive::Integer(num) => compiler.build_push(num as f64),
            Primitive::WholeNumber(num) => compiler.build_push(num as f64),
            Primitive::Variable(var) => compiler.build_push(compiler.target.var(var)),
            Primitive::Event(_) => {
                panic!("events cannot be pushed to the stack");
            }
        }
    }
}

impl StackRepresentable for f64 {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler) {
        if self == 0.0 {
            compiler.write_op(Opcode::PushZero);
        } else {
            compiler.write_op(Opcode::PushNumber);
            compiler.write_u64(self.to_bits());
        }
    }
}

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct ConstantHandle(u32);

impl StackRepresentable for ConstantHandle {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler) {
        compiler.write_op(Opcode::PushConstant);
        compiler.write_imm(self.into());
    }
}

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct VarHandle(u32);

impl StackRepresentable for VarHandle {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler) {
        compiler.write_op(Opcode::PushVar);
        compiler.write_imm(self.into());
    }
}

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct LocalHandle(u32);

impl StackRepresentable for LocalHandle {
    fn build_push_to_stack(self, compiler: &mut ScriptCompiler) {
        compiler.write_op(Opcode::PushLocal);
        compiler.write_imm(self.into());
    }
}

pub trait Label {
    fn write(&self, compiler: &mut ScriptCompiler);
}

#[derive(Debug, From, Into, Clone, Copy, PartialEq, Eq)]
pub struct ConcreteLabel(usize);

impl ConcreteLabel {
    pub fn get(self) -> usize {
        self.0
    }
}

impl Label for ConcreteLabel {
    fn write(&self, compiler: &mut ScriptCompiler) {
        compiler.write_imm(self.0 as u32);
    }
}

#[derive(Debug, Default)]
pub struct PlaceholderLabel {
    pending_usages: RefCell<Vec<ConcreteLabel>>,
}

impl PlaceholderLabel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn finalize_here(self, compiler: &mut ScriptCompiler) {
        let label = compiler.label_here();

        for usage in self.pending_usages.into_inner() {
            compiler.data[usage.get()] = label.0 as u32;
        }
    }
}

impl Label for PlaceholderLabel {
    fn write(&self, compiler: &mut ScriptCompiler) {
        self.pending_usages.borrow_mut().push(compiler.label_here());
        compiler.write_imm(u32::MAX);
    }
}
