use std::{cmp::Ordering, fmt::Debug, sync::Arc};

use bon::bon;
use indexmap::IndexMap;

use crate::{
    codegen::{BlockType, CompileContext, PlaceholderLabel},
    interpreter::{opcode::Opcode, value::Value, RuntimeContext},
};

pub type BlockCompileLogic = dyn Fn(CompileContext<'_>) + Send + Sync;
pub type BlockRuntimeLogic = dyn FnMut(RuntimeContext<'_>) + Send + Sync;

struct LibraryStorage {
    inputs_order: Vec<Arc<str>>,
    compile_logic: Option<Arc<BlockCompileLogic>>,
    runtime_logic: Option<Box<BlockRuntimeLogic>>,
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
        #[builder(start_fn, into)] opcode: Arc<str>,
        #[builder(with = |c: impl Fn(CompileContext<'_>) + Send + Sync + 'static| Arc::new(c))]
        compile_logic: Option<Arc<BlockCompileLogic>>,
        #[builder(with = |c: impl FnMut(RuntimeContext<'_>) + Send + Sync + 'static| Box::new(c))]
        runtime_logic: Option<Box<BlockRuntimeLogic>>,
        #[builder(into, default)] inputs_order: Vec<Arc<str>>,
    ) -> u32 {
        self.register_impl(opcode, compile_logic, runtime_logic, inputs_order, false)
    }

    #[builder(finish_fn = finish)]
    pub fn register_reporter(
        &mut self,

        #[builder(start_fn, into)] opcode: Arc<str>,
        #[builder(with = |c: impl Fn(CompileContext<'_>) + Send + Sync + 'static| Arc::new(c))]
        compile_logic: Option<Arc<BlockCompileLogic>>,
        #[builder(with = |c: impl FnMut(RuntimeContext<'_>) + Send + Sync + 'static| Box::new(c))]
        runtime_logic: Option<Box<BlockRuntimeLogic>>,
        #[builder(into, default)] inputs_order: Vec<Arc<str>>,
    ) -> u32 {
        self.register_impl(opcode, compile_logic, runtime_logic, inputs_order, true)
    }

    fn register_impl(
        &mut self,
        opcode: Arc<str>,
        compile_logic: Option<Arc<BlockCompileLogic>>,
        runtime_logic: Option<Box<BlockRuntimeLogic>>,
        inputs_order: Vec<Arc<str>>,
        is_reporter: bool,
    ) -> u32 {
        let (idx, _) = self.blocks.insert_full(
            opcode,
            LibraryStorage {
                compile_logic,
                runtime_logic,
                inputs_order,
                is_reporter,
            },
        );
        idx as u32
    }

    pub fn split(self) -> (BlockTypeLibrary, BlockRuntimeLibrary) {
        let (type_lib, runtime_lib) = self
            .blocks
            .into_iter()
            .enumerate()
            .map(|(idx, (opcode, storage))| {
                (
                    (
                        opcode.clone(),
                        BlockType {
                            opcode: opcode.clone(),
                            compile_logic: storage.compile_logic.clone(),
                            id: idx as u32,
                            is_reporter: storage.is_reporter,
                            inputs_order: storage.inputs_order,
                        },
                    ),
                    storage.runtime_logic,
                )
            })
            .collect::<(
                IndexMap<Arc<str>, BlockType>,
                Vec<Option<Box<BlockRuntimeLogic>>>,
            )>();

        (
            BlockTypeLibrary { blocks: type_lib },
            BlockRuntimeLibrary {
                blocks: runtime_lib,
            },
        )
    }
}

impl Debug for BlockLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockLibrary({} blocks)", self.blocks.len())
    }
}

impl Default for BlockLibrary {
    fn default() -> Self {
        let mut library = Self::empty();

        library
            .register_block("looks_say")
            .runtime_logic(|mut ctx| {
                let param = ctx.task_mut().pop();
                println!("{}", ctx.program().dbg_string(&param));
            })
            .finish();

        library
            .register_block("data_setvariableto")
            .compile_logic(|ctx| {
                let variable = ctx.block.var_field("VARIABLE");
                let value = &ctx.block.inputs["VALUE"];

                ctx.compiler.build_set_var(variable, value);
                ctx.compiler.build_yield();
            })
            .finish();

        library
            .register_block("data_changevariableby")
            .compile_logic(|ctx| {
                let variable = ctx.block.var_field("VARIABLE");
                let value = &ctx.block.inputs["VALUE"];

                ctx.compiler.build_change_var(variable, value);
                ctx.compiler.build_yield();
            })
            .finish();

        library
            .register_block("control_forever")
            .compile_logic(|ctx| {
                let substack = &ctx.block.inputs["SUBSTACK"];

                let loop_start = ctx.compiler.label_here();
                ctx.compiler.compile_substack(&substack.blocks);
                ctx.compiler.build_jump(loop_start);
            })
            .finish();

        library
            .register_block("control_repeat")
            .compile_logic(|ctx| {
                let times = &ctx.block.inputs["TIMES"];
                let substack = &ctx.block.inputs["SUBSTACK"];

                // Keep track of how many repeats we have remaining
                let repeats_left = ctx.compiler.claim_local();
                ctx.compiler.build_set_local(repeats_left, times);

                let loop_start = ctx.compiler.label_here();
                let loop_end = PlaceholderLabel::new();

                // Do we have any repeats left?
                ctx.compiler.build_cmp(repeats_left, Ordering::Greater, 0.0);
                ctx.compiler.build_jump_if(false, &loop_end);

                // Next iteration
                ctx.compiler.write_op(Opcode::DecLocal);
                ctx.compiler.write_imm(repeats_left.into());

                ctx.compiler.compile_substack(&substack.blocks);

                // Back to start
                ctx.compiler.build_jump(loop_start);

                // Clean up
                ctx.compiler.commit_placeholder(loop_end);
                ctx.compiler.release_local(repeats_left);
            })
            .finish();

        library
            .register_block("control_wait")
            .compile_logic(|ctx| {
                let duration = &ctx.block.inputs["DURATION"];

                ctx.compiler.build_push(duration);
                ctx.compiler.write_op(Opcode::Sleep);
            })
            .finish();

        library
            .register_reporter("operator_join")
            .inputs_order(["STRING1".into(), "STRING2".into()])
            .runtime_logic(|mut ctx| {
                let [str1, str2] = ctx.task_mut().pop_strings();

                let joined = format!("{str1}{str2}");
                ctx.task_mut().push(Value::String(joined.into()));
            })
            .finish();

        library
    }
}

pub struct BlockTypeLibrary {
    blocks: IndexMap<Arc<str>, BlockType>,
}

impl BlockTypeLibrary {
    pub fn block(&self, opcode: &str) -> Option<BlockType> {
        self.blocks
            .get(opcode)
            .cloned()
            .filter(|block| !block.is_reporter)
    }

    pub fn reporter(&self, opcode: &str) -> Option<BlockType> {
        self.blocks
            .get(opcode)
            .cloned()
            .filter(|block| block.is_reporter)
    }
}

impl Debug for BlockTypeLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockTypeLibrary({} blocks)", self.blocks.len())
    }
}

pub struct BlockRuntimeLibrary {
    blocks: Vec<Option<Box<BlockRuntimeLogic>>>,
}

impl BlockRuntimeLibrary {
    pub fn get(&mut self, idx: usize) -> Option<&mut BlockRuntimeLogic> {
        self.blocks[idx].as_deref_mut()
    }
}

impl Debug for BlockRuntimeLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockRuntimeLibrary({} blocks)", self.blocks.len())
    }
}
