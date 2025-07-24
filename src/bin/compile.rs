use std::{collections::HashMap, sync::Arc};

use scratch_vm::{
    ast::{Block, ScratchProject, Script, Sprite, StartCondition, Target, Variable},
    codegen::{BlockLibrary, ScriptCompiler, TargetContext},
    interpreter::{opcode::Trigger, value::ProcedureValue, Program},
};

fn main() {
    let thing_to_type = Variable::new(",v+_??!Fl(Mkx.^9$?aq", "thing_to_type");

    let flag_script = Script {
        start_condition: StartCondition::FlagClicked,
        blocks: vec![
            Block::new("data_setvariableto")
                .with_field("VARIABLE", thing_to_type.clone())
                .with_input("VALUE", Block::text("Hello everyone, Scratch Cat here!")),
            Block::new("looks_say").with_input("MESSAGE", Block::from(thing_to_type.clone())),
        ],
    };

    let library = Arc::new(BlockLibrary::default());
    let target = TargetContext::default();

    let mut compiler = ScriptCompiler::new(target, library.clone());
    compiler.compile(&flag_script);

    dbg!(&compiler);

    let library = Arc::into_inner(compiler.library).unwrap();
    let builtins = library.into_runtime_callbacks();

    let mut program = compiler.target.into_program(builtins);

    let compiled_script = program.register(ProcedureValue::new(
        None,
        0,
        [].into(),
        compiler.data.into_boxed_slice(),
    ));
    program.add_trigger(compiled_script, Trigger::OnStart);

    program.dispatch(Trigger::OnStart);
    program.run();
}
