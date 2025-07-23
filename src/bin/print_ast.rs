use std::collections::HashMap;

use scratch_vm::ast::{
    Block, Field, Input, ProcedureArgument, ProcedurePrototype, ScratchProject, Script, Sprite,
    StartCondition, Target, Variable,
};

fn main() {
    let textsofar = Variable::new("yz?6:NTcXFFE%NTLQ!@G", "textsofar");
    let c = Variable::new("ziG}OJfdDo2+FuOH7^4=", "c");

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
    let flag_script = Script {
        start_condition: StartCondition::FlagClicked,
        blocks: vec![
            Block::new("looks_hide"),
            Block::new("looks_setsizeto").with_input("SIZE", Block::number("100")),
            Block::new("looks_cleargraphiceffects"),
        ],
    };

    let ast = ScratchProject {
        events: vec![],
        targets: vec![Target {
            name: "Cat".into(),
            scripts: vec![
                flag_script,
                type_script,
            ],
            sprite: Some(Sprite {}),
            variables: HashMap::from([]),
        }],
    };

    dbg!(ast);
}
