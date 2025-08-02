use std::collections::HashMap;

use scratch_vm::{ast::{
    project::ScratchProject, Block, Event, ProcedureArgument, ProcedurePrototype, Script, Sprite, StartCondition, Target, Variable, VariableRef
}, interpreter::value::Value};

fn main() {
    let thingtotype = VariableRef::new(",v+_??!Fl(Mkx.^9$?aq", "thingtotype");
    let textsofar = VariableRef::new("yz?6:NTcXFFE%NTLQ!@G", "textsofar");
    let c = VariableRef::new("ziG}OJfdDo2+FuOH7^4=", "c");

    let typeit = Event::new("O#Gthx*c8wNEzk@GC}f2", "typeit");

    #[rustfmt::skip]
    let repeat_times = Block::new("operator_length")
        .with_input(
            "STRING",
            Block::param("text"),
        );

    let repeat_substack = vec![
        Block::new("data_changevariableby")
            .with_field("VARIABLE", c.clone())
            .with_input("VALUE", Block::number("1")),
        Block::new("looks_say").with_input(
            "MESSAGE",
            Block::new("operator_join")
                .with_input("STRING1", Block::from(textsofar.clone()))
                .with_input(
                    "STRING2",
                    Block::new("operator_letter_of")
                        .with_input("LETTER", Block::from(c.clone()))
                        .with_input("STRING", Block::param("text")),
                ),
        ),
        Block::new("data_setvariableto")
            .with_field("VARIABLE", textsofar.clone())
            .with_input(
                "VALUE",
                Block::new("operator_join")
                    .with_input("STRING1", Block::from(textsofar.clone()))
                    .with_input(
                        "STRING2",
                        Block::new("operator_letter_of")
                            .with_input("LETTER", Block::from(c.clone()))
                            .with_input("STRING", Block::param("text")),
                    ),
            ),
    ];

    let type_script = Script {
        start_condition: StartCondition::ProcedureCalled(
            ProcedurePrototype::new("type %s")
                .with_arg(ProcedureArgument::new("|n@zQ};g|LMYr1LJOEmI", "text")),
        ),
        blocks: vec![
            Block::new("data_setvariableto")
                .with_field("VARIABLE", textsofar.clone())
                .with_input("VALUE", Block::text("")),
            Block::new("data_setvariableto")
                .with_field("VARIABLE", c.clone())
                .with_input("VALUE", Block::text("0")),
            Block::new("control_repeat")
                .with_input("TIMES", repeat_times)
                .with_input("SUBSTACK", repeat_substack),
        ],
    };

    let event_script = Script {
        start_condition: StartCondition::BroadcastReceived(typeit.clone()),
        blocks: vec![
            Block::call("type %s")
                .with_input("|n@zQ};g|LMYr1LJOEmI", Block::from(thingtotype.clone())),
        ],
    };

    let flag_script = Script {
        start_condition: StartCondition::FlagClicked,
        blocks: vec![
            Block::new("data_setvariableto")
                .with_field("VARIABLE", thingtotype.clone())
                .with_input("VALUE", Block::text("Hello everyone, Scratch Cat here!")),
            Block::new("event_broadcast")
                .with_input("BROADCAST_INPUT", Block::from(typeit.clone())),
        ],
    };

    let ast = ScratchProject {
        events: vec![],
        targets: vec![Target {
            name: "Cat".into(),
            scripts: vec![flag_script, event_script, type_script],
            sprite: Some(Sprite {}),
            variables: HashMap::from([]),
        }],
        global_vars: HashMap::from([
            ("001".into(), Variable::new(thingtotype, Value::default())),
            ("002".into(), Variable::new(textsofar, Value::default())),
            ("003".into(), Variable::new(c, Value::default())),
        ]),
    };

    dbg!(ast);
}
