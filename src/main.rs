use scratch_vm::interpreter::{
    Program,
    opcode::{BuiltinProcedure, Opcode, Trigger},
    value::{Local, Procedure, Value, Var},
};

fn main() {
    let mut program = Program::new(
        [Value::String("hello everyone...".into())].into(),
        [
            Var::new("thingtotype"),
            Var::new("textsofar"),
            Var::new("c"),
        ]
        .into(),
    );

    let typeit = program.register_event("typeit");

    let type_proc = program.register(Procedure::new(
        Some("type %s".into()),
        1,
        [Local::from("text"), Local::from("repeats_remaining_0")].into(),
        {
            let mut instructions = vec![
                // Clear out state
                Opcode::ClearVar as _,
                1,
                Opcode::ZeroVar as _,
                2,
                // Get length of `text`, store in repeats counter
                Opcode::PushLocal as _,
                0,
                Opcode::CallBuiltin as _,
                BuiltinProcedure::LengthOf as _,
                Opcode::SetLocal as _,
                1,
            ];
            let repeat_0 = instructions.len();
            instructions.extend([
                // Do we have any repeats left?
                Opcode::PushLocal as _,
                1,
                Opcode::PushZero as _,
                Opcode::GreaterThan as _,
                Opcode::JumpIfFalse as _,
            ]);
            let jump_dest = instructions.len();
            instructions.extend([
                0, // placeholder
                // Next iteration...
                Opcode::DecLocal as _,
                1,
                // Loop logic end
                // c += 1
                Opcode::PushUnsignedInt as _,
                1,
                Opcode::PushVar as _,
                2,
                Opcode::Add as _,
                Opcode::SetVar as _,
                2,
                //
                // say(join(textsofar, letter_of(c, text)))
                Opcode::PushVar as _,
                1,
                // letter_of
                Opcode::PushVar as _,
                2,
                Opcode::PushLocal as _,
                0,
                Opcode::CallBuiltin as _,
                BuiltinProcedure::LetterOf as _,
                // join
                Opcode::CallBuiltin as _,
                BuiltinProcedure::Join as _,
                // say
                Opcode::CallBuiltin as _,
                BuiltinProcedure::Say as _,
                //
                // textsofar = join(textsofar, letter_of(c, text))
                Opcode::PushVar as _,
                1,
                Opcode::PushVar as _,
                2,
                Opcode::PushLocal as _,
                0,
                Opcode::CallBuiltin as _,
                BuiltinProcedure::LetterOf as _,
                Opcode::CallBuiltin as _,
                BuiltinProcedure::Join as _,
                Opcode::SetVar as _,
                1,
                //
                // Loop logic again
                Opcode::Jump as _,
                repeat_0 as u32,
            ]);

            let repeat_0_end = instructions.len();
            // Patch jump from earlier
            instructions[jump_dest] = repeat_0_end as u32;

            instructions.extend([Opcode::Return as u32]);

            instructions.into_boxed_slice()
        },
    ));

    let typeit_handler = program.register(Procedure::new(
        Some("say_hi".into()),
        0,
        [].into(),
        [
            Opcode::PushVar as _,
            0,
            Opcode::CallProcedure as _,
            type_proc.id().get() as _,
            Opcode::Return as _,
        ]
        .into(),
    ));
    program.add_trigger(typeit_handler, Trigger::Event(typeit));

    let main = program.register(Procedure::new(
        Some("main".into()),
        0,
        [].into(),
        [
            Opcode::PushConstant as _,
            0,
            Opcode::SetVar as _,
            0,
            Opcode::DispatchEvent as _,
            typeit.get() as _,
            Opcode::Return as _,
        ]
        .into(),
    ));
    program.add_trigger(main, Trigger::OnStart);

    program.dispatch(Trigger::OnStart);
    program.run();
}
