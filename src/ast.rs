use std::{collections::HashMap, rc::Rc};

use derive_more::{Constructor, From, Into};

use crate::interpreter::value::Value;

// pub mod primitives;

#[derive(Debug)]
pub struct ScratchProject {
    pub targets: Vec<Target>,
    pub events: Vec<Event>,
}

#[derive(Debug)]
pub struct Target {
    pub name: Rc<str>,
    pub scripts: Vec<Script>,
    pub variables: HashMap<Rc<str>, Value>,
    pub sprite: Option<Sprite>,
}

#[derive(Debug)]
pub struct Sprite {}

#[derive(Debug)]
pub enum StartCondition {
    FlagClicked,
    BroadcastReceived(Event),
    ProcedureCalled(ProcedurePrototype),
}

#[derive(Debug)]
pub struct ProcedurePrototype {
    pub proc_code: Rc<str>,
    pub arguments: Vec<ProcedureArgument>,
    pub skip_yields: bool,
}

impl ProcedurePrototype {
    pub fn new(proc_code: impl Into<Rc<str>>) -> Self {
        Self {
            proc_code: proc_code.into(),
            arguments: vec![],
            skip_yields: false,
        }
    }

    pub fn with_arg(mut self, arg: ProcedureArgument) -> Self {
        self.arguments.push(arg);
        self
    }
}

#[derive(Debug)]
pub struct ProcedureArgument {
    pub id: Rc<str>,
    pub name: Rc<str>,
    pub default: Rc<str>,
}

impl ProcedureArgument {
    pub fn new(id: impl Into<Rc<str>>, name: impl Into<Rc<str>>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            default: "".into(),
        }
    }

    pub fn with_default(mut self, default: impl Into<Rc<str>>) -> Self {
        self.default = default.into();
        self
    }
}

#[derive(Debug)]
pub struct Script {
    pub start_condition: StartCondition,
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub struct Block {
    pub opcode: Rc<str>,
    pub proc_code: Option<Rc<str>>,
    /// Inputs that reference other blocks
    pub inputs: HashMap<Rc<str>, Input>,
    /// Inputs that don't reference other blocks
    pub fields: HashMap<Rc<str>, Field>,
}

impl Block {
    pub fn new(opcode: impl Into<Rc<str>>) -> Self {
        Self {
            opcode: opcode.into(),
            proc_code: None,
            inputs: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    pub fn call(proc_code: impl Into<Rc<str>>) -> Self {
        Self {
            opcode: "procedures_call".into(),
            proc_code: Some(proc_code.into()),
            inputs: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    fn new_number(opcode: impl Into<Rc<str>>, num: impl Into<Rc<str>>) -> Self {
        Self::new(opcode).with_field("NUM", Field::new(num))
    }

    pub fn text(text: impl Into<Rc<str>>) -> Self {
        Self::new("text").with_field("TEXT", Field::new(text))
    }

    pub fn number(num: impl Into<Rc<str>>) -> Self {
        Self::new_number("math_number", num)
    }

    pub fn integer(num: impl Into<Rc<str>>) -> Self {
        Self::new_number("math_integer", num)
    }

    pub fn whole_number(num: impl Into<Rc<str>>) -> Self {
        Self::new_number("math_whole_number", num)
    }

    pub fn pos_number(num: impl Into<Rc<str>>) -> Self {
        Self::new_number("math_positive_number", num)
    }

    pub fn angle(num: impl Into<Rc<str>>) -> Self {
        Self::new_number("math_angle", num)
    }

    pub fn var(id: impl Into<Rc<str>>, name: impl Into<Rc<str>>) -> Self {
        Block::new("data_variable").with_field("VARIABLE", Field::identified(id, name))
    }

    pub fn param(name: impl Into<Rc<str>>) -> Self {
        Block::new("argument_reporter_string_number").with_field("VALUE", Field::new(name))
    }

    pub fn event(id: impl Into<Rc<str>>, name: impl Into<Rc<str>>) -> Self {
        Block::new("event_broadcast_menu")
            .with_field("BROADCAST_OPTION", Field::identified(id, name))
    }

    pub fn with_input(mut self, name: impl Into<Rc<str>>, input: impl Into<Input>) -> Self {
        self.inputs.insert(name.into(), input.into());
        self
    }

    pub fn with_field(mut self, name: impl Into<Rc<str>>, field: impl Into<Field>) -> Self {
        self.fields.insert(name.into(), field.into());
        self
    }
}

impl From<Variable> for Block {
    fn from(value: Variable) -> Self {
        Self::var(value.0.id, value.0.name)
    }
}

impl From<Event> for Block {
    fn from(value: Event) -> Self {
        Self::event(value.0.id, value.0.name)
    }
}

#[derive(Debug)]
pub struct Input {
    pub blocks: Vec<Block>,
    pub shadow: Option<Block>,
}

impl Input {}

impl From<Block> for Input {
    fn from(value: Block) -> Self {
        Self {
            blocks: vec![value],
            shadow: None,
        }
    }
}

impl From<Vec<Block>> for Input {
    fn from(value: Vec<Block>) -> Self {
        Self {
            blocks: value,
            shadow: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    pub value: Rc<str>,
    pub id: Option<Rc<str>>,
}

impl Field {
    pub fn new(value: impl Into<Rc<str>>) -> Self {
        Self {
            value: value.into(),
            id: None,
        }
    }

    pub fn identified(id: impl Into<Rc<str>>, value: impl Into<Rc<str>>) -> Self {
        Self {
            value: value.into(),
            id: Some(id.into()),
        }
    }

    pub fn to_named_resource(self) -> Option<NamedResource> {
        self.id.map(|id| NamedResource {
            name: self.value,
            id,
        })
    }
}

impl<T: Into<NamedResource>> From<T> for Field {
    fn from(value: T) -> Self {
        let resource: NamedResource = value.into();
        Self::identified(resource.id, resource.name)
    }
}

#[derive(Debug, Clone, Constructor)]
pub struct NamedResource {
    pub id: Rc<str>,
    pub name: Rc<str>,
}

#[derive(Debug, Clone, From, Into)]
pub struct Variable(NamedResource);

impl Variable {
    pub fn new(id: impl Into<Rc<str>>, name: impl Into<Rc<str>>) -> Self {
        Self(NamedResource::new(id.into(), name.into()))
    }

    pub fn into_inner(self) -> NamedResource {
        self.0
    }
}

#[derive(Debug, Clone, From, Into)]
pub struct Event(NamedResource);

impl Event {
    pub fn new(id: impl Into<Rc<str>>, name: impl Into<Rc<str>>) -> Self {
        Self(NamedResource::new(id.into(), name.into()))
    }

    pub fn into_inner(self) -> NamedResource {
        self.0
    }
}
