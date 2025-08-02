use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    thread::{self, Scope, scope},
};

use indexmap::{IndexMap, IndexSet};

use crate::{
    ast::{Block, Event, StartCondition, Target, Variable},
    blocks::BlockLibrary,
    codegen::{ProjectContext, ScriptCompiler, TargetCodegenContext},
    interpreter::{
        Program, TargetScope,
        opcode::Trigger,
        value::{EventValue, Local, ProcedureValue, Value},
    },
};

#[derive(Debug)]
pub struct ScratchProject {
    pub targets: Vec<Target>,
    pub events: IndexMap<Arc<str>, Event>,
    pub global_vars: HashMap<Arc<str>, Variable>,
}

impl ScratchProject {
    pub fn compile(&self) -> Program {
        let (type_library, rt_library) = BlockLibrary::default().split();
        let type_library = Arc::new(type_library);

        // Finding all the text constants ahead of time allows us to parallelize script compilation
        // because then we don't need to assign new indexes to constants on the fly and share that
        // mutable state across threads.
        let text_constants = self.find_text_constants();
        let project_ctx = Arc::new(ProjectContext::new(
            self.global_vars.values().cloned(),
            text_constants.clone(),
        ));

        let global_vars = self.global_vars.values().map(|v| v.initialize()).collect();
        let event_values = self
            .events
            .values()
            .map(|e| EventValue::new(e.name()))
            .collect();

        // It's important this is in the same order as our targets because a target_id is used as an index
        // into this list of scopes by the interpreter.
        let target_scopes = self.targets.iter().map(TargetScope::from).collect();

        scope(|scope| {
            let mut target_tasks = Vec::new();

            for (target_id, target) in self.targets.iter().enumerate() {
                let project_ctx = project_ctx.clone();
                let type_library = type_library.clone();

                let task = scope.spawn(move || {
                    let mut initial_vars = self.global_vars.clone();
                    initial_vars.extend(target.variables.clone());

                    let ctx = Arc::new(TargetCodegenContext::new(
                        project_ctx,
                        initial_vars.values().cloned(),
                    ));

                    let mut compile_tasks = Vec::new();

                    for (script_id, script) in target.scripts.iter().enumerate() {
                        let ctx = ctx.clone();
                        let type_library = type_library.clone();

                        let task = scope.spawn(move || {
                            let proc_info = script
                                .start_condition
                                .try_unwrap_procedure_called_ref()
                                .ok();
                            let param_count = proc_info.map_or(0, |proto| proto.arguments.len());

                            // If this script is going to run in warp mode, there's no reason to generate yields
                            // because they will be ignored at runtime.
                            let warp_enabled = proc_info.is_some_and(|p| p.warp);

                            let mut compiler =
                                ScriptCompiler::new(ctx, type_library, warp_enabled, param_count);
                            compiler.compile(script);

                            let name = format!("Script {script_id} of Target {}", target.name);

                            let proc = ProcedureValue::new(
                                Some(name.into()),
                                target_id,
                                param_count,
                                compiler.get_locals(),
                                compiler.data.into_boxed_slice(),
                                warp_enabled,
                            );

                            let trigger = match &script.start_condition {
                                StartCondition::FlagClicked => Some(Trigger::OnStart),
                                StartCondition::BroadcastReceived(event) => {
                                    let idx = self
                                        .events
                                        .get_index_of(&event.id())
                                        .expect("event should have index");
                                    Some(Trigger::Event(idx.into()))
                                }
                                StartCondition::ProcedureCalled(_proto) => None,
                            };

                            (trigger, proc)
                        });

                        compile_tasks.push(task);
                    }

                    compile_tasks
                });

                target_tasks.push(task);
            }

            let mut program = Program::new(
                rt_library,
                text_constants.iter().cloned().map(Value::String).collect(),
                event_values,
                global_vars,
                target_scopes,
            );

            for task in target_tasks {
                let compile_tasks = task.join().unwrap();

                for task in compile_tasks {
                    let (trigger, proc) = task.join().unwrap();

                    let handle = program.register(proc);
                    if let Some(trigger) = trigger {
                        program.add_trigger(handle, trigger);
                    }
                }
            }

            program
        })
    }

    fn find_text_constants(&self) -> Arc<IndexSet<Arc<str>>> {
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
                    traverse_substack(substack, constants);
                }
            }
        }

        for target in &self.targets {
            for script in &target.scripts {
                traverse_substack(&script.blocks, &mut constants);
            }
        }

        Arc::new(constants)
    }
}
