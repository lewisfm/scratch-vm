use std::{collections::HashMap, sync::Arc};

use scratch_vm::{ast::{Block, ScratchProject, Script, Sprite, StartCondition, Target, Variable}, codegen::{BlockLibrary, ScriptCompiler, TargetContext}};


fn main() {
    let thingtotype = Variable::new(",v+_??!Fl(Mkx.^9$?aq", "thingtotype");

    let flag_script = Script {
        start_condition: StartCondition::FlagClicked,
        blocks: vec![
            Block::new("data_setvariableto")
                .with_field("VARIABLE", thingtotype.clone())
                .with_input("VALUE", Block::text("Hello everyone, Scratch Cat here!")),
        ],
    };

    // let ast = ScratchProject {
    //     events: vec![],
    //     targets: vec![Target {
    //         name: "Cat".into(),
    //         scripts: vec![flag_script],
    //         sprite: Some(Sprite {}),
    //         variables: HashMap::from([]),
    //     }],
    // };

    let library = Arc::new(BlockLibrary::default());
    let target = TargetContext::default();

    let mut compiler = ScriptCompiler::new(target, library);
    compiler.compile(&flag_script);
    dbg!(compiler);
}
