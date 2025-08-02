use std::{collections::HashMap, fmt::Display, mem::take, rc::Rc, sync::Arc};

use serde::{Deserialize, Serialize, de::value};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::{
    ast::{
        Block, Event, Field, Input, Script, Target, Variable, VariableRef, project::ScratchProject,
    },
    interpreter::value::Value,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Sb3Project {
    pub targets: Vec<Sb3Target>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sb3Target {
    is_stage: bool,
    name: Arc<str>,
    variables: HashMap<Arc<str>, Sb3Variable>,
    broadcasts: HashMap<Arc<str>, Arc<str>>,
    blocks: HashMap<Arc<str>, Sb3Block>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sb3Variable(Arc<str>, Sb3Value);

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sb3Value {
    String(Arc<str>),
    Number(f64),
}

impl From<Sb3Value> for Value {
    fn from(value: Sb3Value) -> Self {
        match value {
            Sb3Value::Number(num) => Self::Number(num),
            Sb3Value::String(str) => Self::String(str),
        }
    }
}

impl Display for Sb3Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::String(str) => str.to_string(),
            Self::Number(num) => num.to_string(),
        };
        write!(f, "{str}")
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sb3Block {
    opcode: Arc<str>,
    next: Option<Arc<str>>,
    parent: Option<Arc<str>>,
    inputs: HashMap<Arc<str>, Sb3Input>,
    fields: HashMap<Arc<str>, Sb3Field>,
    top_level: bool,
    mutation: Option<Sb3Mutation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sb3Mutation {
    #[serde(rename = "proccode")]
    proc_code: Arc<str>,
    #[serde(flatten)]
    prototype: Option<Sb3MutationPrototype>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sb3MutationPrototype {
    #[serde(rename = "argumentdefaults")]
    argument_defaults: Arc<str>,
    warp: Arc<str>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sb3BlockRef {
    Ref(Arc<str>),
    InlinePrimitive(Sb3InlineBlock),
}

impl Sb3BlockRef {
    fn take_inner(self, blocks: &mut HashMap<Arc<str>, Sb3Block>) -> Vec<Block> {
        match self {
            Self::Ref(block_id) => deserialize_substack(block_id, blocks),
            Self::InlinePrimitive(block) => vec![block.into()],
        }
    }
}

// https://github.com/scratchfoundation/scratch-editor/blob/f964757a9559dd604b9f2474ed5b744cba8766cc/packages/scratch-vm/src/serialization/sb3.js#L104
#[derive(Debug, Serialize, Deserialize)]
pub struct Sb3InlineBlock(
    Sb3InlineBlockType,
    Arc<str>,
    #[serde(default)] Option<Arc<str>>,
);

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Sb3InlineBlockType {
    Number = 4,
    PositiveNumber = 5,
    WholeNumber = 6,
    Integer = 7,
    Angle = 8,
    Color = 9,
    Text = 10,
    Broadcast = 11,
    Variable = 12,
    List = 13,
}

// https://github.com/scratchfoundation/scratch-editor/blob/f964757a9559dd604b9f2474ed5b744cba8766cc/packages/scratch-vm/src/serialization/sb3.js#L138
#[derive(Debug, Serialize, Deserialize)]
pub struct Sb3Input(
    Sb3InputStatus,
    Sb3BlockRef,
    #[serde(default)] Option<Sb3BlockRef>,
);

#[derive(Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum Sb3InputStatus {
    ShadowOnly = 1,
    NormalOnly = 2,
    NormalAndShadow = 3,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Sb3Field {
    Full {
        name: Arc<str>,
        value: Arc<str>,
        id: Option<Arc<str>>,
    },
    Compact(Arc<str>, Option<Arc<str>>),
}

impl From<Sb3Field> for Field {
    fn from(value: Sb3Field) -> Self {
        match value {
            Sb3Field::Compact(value, id) => Field { id, value },
            Sb3Field::Full { value, id, .. } => Field { id, value },
        }
    }
}

impl From<Sb3Project> for ScratchProject {
    fn from(mut project: Sb3Project) -> Self {
        let mut stage = project
            .targets
            .iter_mut()
            .find(|t| t.is_stage)
            .expect("project must have stage");

        let events = stage
            .broadcasts
            .iter()
            .map(|(id, name)| (id.clone(), Event::new(id.clone(), name.clone())))
            .collect();

        let global_vars = deserialize_variables(&mut stage);

        let targets = project
            .targets
            .into_iter()
            .map(|mut t| {
                let scripts = build_scripts(&mut t);
                let variables = deserialize_variables(&mut t);

                Target {
                    name: t.name,
                    variables,
                    sprite: None,
                    scripts,
                }
            })
            .collect();

        Self { events, targets, global_vars }
    }
}

fn deserialize_variables(target: &mut Sb3Target) -> HashMap<Arc<str>, Variable> {
    target
        .variables
        .drain()
        .map(|(id, var)| {
            let var = Variable::new(VariableRef::new(id.clone(), var.0), var.1.into());
            (id, var)
        })
        .collect()
}

impl From<Sb3InlineBlock> for Block {
    fn from(value: Sb3InlineBlock) -> Self {
        let id = value.2;
        let inner = value.1;

        match value.0 {
            Sb3InlineBlockType::Number => Block::number(inner),
            Sb3InlineBlockType::PositiveNumber => Block::pos_number(inner),
            Sb3InlineBlockType::WholeNumber => Block::whole_number(inner),
            Sb3InlineBlockType::Integer => Block::integer(inner),
            Sb3InlineBlockType::Angle => Block::angle(inner),
            Sb3InlineBlockType::Color => Block::color(inner),
            Sb3InlineBlockType::Text => Block::text(inner),
            Sb3InlineBlockType::Broadcast => Block::event(id.unwrap(), inner),
            Sb3InlineBlockType::Variable => Block::var(id.unwrap(), inner),
            Sb3InlineBlockType::List => unimplemented!(),
        }
    }
}

fn build_scripts(target: &mut Sb3Target) -> Vec<Script> {
    let mut scripts = vec![];

    let top_level_block_ids = target
        .blocks
        .iter()
        .filter(|(_id, block)| block.top_level)
        .map(|(id, _block)| id.clone())
        .collect::<Vec<_>>();

    for block_id in top_level_block_ids {
        let mut substack = deserialize_substack(block_id, &mut target.blocks);

        let Some(start_condition) = substack[0].try_as_start_condition() else {
            eprintln!("WARN: Script missing start condition");
            eprintln!(
                "    > Triggered by top-level block {:?}",
                substack[0].opcode
            );
            continue;
        };

        substack.remove(0);

        scripts.push(Script {
            start_condition,
            blocks: substack,
        });
    }

    scripts
}

fn deserialize_substack(start_id: Arc<str>, blocks: &mut HashMap<Arc<str>, Sb3Block>) -> Vec<Block> {
    let mut substack = vec![];
    let mut next_id = Some(start_id);

    while let Some(id) = next_id {
        let mut block = blocks.remove(&id).expect("missing block");
        substack.push(deserialize_block(&mut block, blocks));
        next_id = block.next;
    }

    substack
}

fn deserialize_block(block: &mut Sb3Block, other_blocks: &mut HashMap<Arc<str>, Sb3Block>) -> Block {
    let fields = take(&mut block.fields)
        .into_iter()
        .map(|(name, f)| (name, f.into()))
        .collect();

    let inputs = take(&mut block.inputs)
        .into_iter()
        .map(|(name, input)| {
            let blocks = input.1.take_inner(other_blocks);

            let shadow = input.2.map(|shadow| {
                let mut substack = shadow.take_inner(other_blocks);
                assert!(substack.len() == 1, "shadows cannot be substacks");

                substack.remove(0)
            });

            (name, Input { blocks, shadow })
        })
        .collect();

    Block {
        opcode: take(&mut block.opcode),
        proc_code: block
            .mutation
            .as_ref()
            .map(|mutation| mutation.proc_code.clone()),
        fields,
        inputs,
    }
}
