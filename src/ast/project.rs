use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    thread::{self, Scope, scope},
};

use indexmap::IndexSet;

use crate::{
    ast::{Block, Event, Target, Variable},
    codegen::{BlockLibrary, ProjectContext, ScriptCompiler, TargetContext},
    interpreter::{Program, value::Value},
};

#[derive(Debug)]
pub struct ScratchProject {
    pub targets: Vec<Target>,
    pub events: Vec<Event>,
    pub global_vars: HashMap<Arc<str>, Variable>,
}

impl ScratchProject {
    pub fn compile(&self) -> Program {
        let library = Arc::new(BlockLibrary::default());
        let text_constants = self.find_text_constants();
        let project_ctx = Arc::new(ProjectContext::new(
            self.global_vars.values().cloned(),
            text_constants.clone(),
        ));

        scope(|scope| {
            for target in &self.targets {
                scope.spawn(|| {
                    let mut initial_vars = self.global_vars.clone();
                    initial_vars.extend(target.variables.clone());

                    let ctx = Arc::new(TargetContext::new(
                        project_ctx.clone(),
                        initial_vars.values().cloned(),
                    ));

                    for script in &target.scripts {
                        let ctx = ctx.clone();
                        let library = library.clone();

                        scope.spawn(move || {
                            let mut compiler = ScriptCompiler::new(ctx, library);
                            compiler.compile(script);


                        });
                    }
                });
            }
        });

        todo!()
    }

    fn find_text_constants(&self) -> IndexSet<Arc<str>> {
        let mut constants = IndexSet::new();

        fn traverse_substack(stack: &[Block], constants: &mut IndexSet<Arc<str>>) {
            for block in stack {
                // If this is a text block, add it to the constant pool
                if let Some(primitive) = block.try_as_primitive()
                    && let Ok(text) = primitive.try_unwrap_text()
                {
                    constants.insert(text);
                }

                // (Otherwise,) find child blocks that might be text
                for input in block.inputs.values() {
                    let substack = &input.blocks;
                    traverse_substack(&substack, constants);
                }
            }
        }

        for target in &self.targets {
            for script in &target.scripts {
                traverse_substack(&script.blocks, &mut constants);
            }
        }

        constants
    }
}
