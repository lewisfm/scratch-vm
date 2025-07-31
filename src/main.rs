use std::{env::args, fs};

use scratch_vm::{ast::project::ScratchProject, codegen::BlockLibrary, interpreter::{
    opcode::{BuiltinProcedure, Opcode, Trigger}, value::{Local, ProcedureValue, Value, VarState}, Program
}, sb3::Sb3Project};

fn main() {
    let args = args().collect::<Vec<_>>();
    let sb3_file = fs::read_to_string(&args[1]).unwrap();

    let sb3: Sb3Project = serde_json::from_str(&sb3_file).unwrap();
    let project = ScratchProject::from(sb3);
    let mut program = project.compile();

    program.dispatch(Trigger::OnStart);
    program.run();
}
