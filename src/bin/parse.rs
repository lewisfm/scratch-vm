use scratch_vm::{ast::project::ScratchProject, sb3::Sb3Project};

fn main() {
    let sb3 = include_str!("../../lang/project.json");

    let sb3: Sb3Project = serde_json::from_str(sb3).unwrap();
    let project = ScratchProject::from(sb3);

    println!("{project:#?}");
}
