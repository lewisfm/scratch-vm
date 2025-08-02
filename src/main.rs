use std::{env::args, fs, process::exit};

use scratch_vm::{ast::project::ScratchProject, interpreter::opcode::Trigger, sb3::Sb3Project};

fn main() {
    let args = args().collect::<Vec<_>>();
    let Some(sb3_path) = args.get(1) else {
        print_usage();
    };

    let sb3_file = fs::read_to_string(sb3_path).unwrap();

    let sb3: Sb3Project = serde_json::from_str(&sb3_file).unwrap();
    let project = ScratchProject::from(sb3);
    eprintln!("project: {project:#?}");
    let mut program = project.compile();
    eprintln!("program: {program:#?}");

    program.dispatch(Trigger::OnStart);

    while program.has_incomplete_tasks() {
        program.run_frame();
    }
}

fn print_usage() -> ! {
    eprintln!("\nUsage: scratch-vm <PATH-TO-SB3>");
    exit(1);
}
