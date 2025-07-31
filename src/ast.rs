use std::{any::type_name, collections::HashMap, rc::Rc, str::FromStr, sync::Arc};

use derive_more::{AsRef, Constructor, From, Into, TryUnwrap, Unwrap};

use crate::interpreter::value::Value;

// pub mod primitives;
pub mod project;

#[derive(Debug)]
pub struct Target {
    pub name: Arc<str>,
    pub scripts: Vec<Script>,
    pub variables: HashMap<Arc<str>, Variable>,
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
    pub proc_code: Arc<str>,
    pub arguments: Vec<ProcedureArgument>,
    pub skip_yields: bool,
}

impl ProcedurePrototype {
    pub fn new(proc_code: impl Into<Arc<str>>) -> Self {
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
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub default: Arc<str>,
}

impl ProcedureArgument {
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            default: "".into(),
        }
    }

    pub fn with_default(mut self, default: impl Into<Arc<str>>) -> Self {
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
    pub opcode: Arc<str>,
    pub proc_code: Option<Arc<str>>,
    /// Inputs that reference other blocks
    pub inputs: HashMap<Arc<str>, Input>,
    /// Inputs that don't reference other blocks
    pub fields: HashMap<Arc<str>, Field>,
}

impl Block {
    pub const TEXT_FIELD: &str = "TEXT";
    pub const NUM_FIELD: &str = "TEXT";
    pub const COLOR_FIELD: &str = "COLOUR";
    pub const VAR_FIELD: &str = "VARIABLE";
    pub const ARG_NAME_FIELD: &str = "VALUE";
    pub const EVENT_FIELD: &str = "BROADCAST_OPTION";

    pub const TEXT: &str = "text";
    pub const NUMBER: &str = "math_number";
    pub const INTEGER: &str = "math_integer";
    pub const WHOLE_NUMBER: &str = "math_whole_number";
    pub const POSITIVE_NUMBER: &str = "math_positive_number";
    pub const ANGLE: &str = "math_angle";
    pub const COLOR: &str = "colour_picker";
    pub const VARIABLE: &str = "data_variable";
    pub const STRING_ARG: &str = "argument_reporter_string_number";
    pub const BOOL_ARG: &str = "argument_reporter_boolean";
    pub const EVENT: &str = "event_broadcast_menu";

    pub const PROCECURE_DEFN_PROTOTYPE: &str = "custom_block";

    pub fn new(opcode: impl Into<Arc<str>>) -> Self {
        Self {
            opcode: opcode.into(),
            proc_code: None,
            inputs: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    pub fn call(proc_code: impl Into<Arc<str>>) -> Self {
        Self {
            opcode: "procedures_call".into(),
            proc_code: Some(proc_code.into()),
            inputs: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    fn new_number(opcode: impl Into<Arc<str>>, num: impl Into<Arc<str>>) -> Self {
        Self::new(opcode).with_field("NUM", Field::simple(num))
    }

    pub fn text(text: impl Into<Arc<str>>) -> Self {
        Self::new("text").with_field("TEXT", Field::simple(text))
    }

    pub fn number(num: impl Into<Arc<str>>) -> Self {
        Self::new_number("math_number", num)
    }

    pub fn integer(num: impl Into<Arc<str>>) -> Self {
        Self::new_number("math_integer", num)
    }

    pub fn whole_number(num: impl Into<Arc<str>>) -> Self {
        Self::new_number("math_whole_number", num)
    }

    pub fn pos_number(num: impl Into<Arc<str>>) -> Self {
        Self::new_number("math_positive_number", num)
    }

    pub fn angle(num: impl Into<Arc<str>>) -> Self {
        Self::new_number("math_angle", num)
    }

    pub fn color(num: impl Into<Arc<str>>) -> Self {
        Self::new_number("math_angle", num)
    }

    pub fn var(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
        Block::new("data_variable").with_field("VARIABLE", Field::identified(id, name))
    }

    pub fn param(name: impl Into<Arc<str>>) -> Self {
        Block::new("argument_reporter_string_number").with_field("VALUE", Field::simple(name))
    }

    pub fn event(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
        Block::new("event_broadcast_menu")
            .with_field("BROADCAST_OPTION", Field::identified(id, name))
    }

    pub fn with_input(mut self, name: impl Into<Arc<str>>, input: impl Into<Input>) -> Self {
        self.inputs.insert(name.into(), input.into());
        self
    }

    pub fn with_field(mut self, name: impl Into<Arc<str>>, field: impl Into<Field>) -> Self {
        self.fields.insert(name.into(), field.into());
        self
    }

    pub fn simple_field(&self, name: &str) -> Arc<str> {
        if let Some(field) = self.fields.get(name)
            && field.id.is_none()
        {
            field.value.clone()
        } else {
            panic!(
                "block {:?} must have a simple field named {name:?}",
                self.opcode
            );
        }
    }

    pub fn parsed_field<T: FromStr>(&self, name: &str) -> T {
        if let Some(field) = self.fields.get(name)
            && field.id.is_none()
        {
            if let Ok(parsed) = field.value.parse() {
                parsed
            } else {
                panic!(
                    "field {name:?} in block {:?} was not a valid {}",
                    self.opcode,
                    type_name::<T>()
                )
            }
        } else {
            panic!(
                "block {:?} must have a simple field named {name:?}",
                self.opcode
            );
        }
    }

    pub fn identified_field(&self, name: &str) -> NamedResource {
        if let Some(field) = self.fields.get(name)
            && let Some(id) = field.id.clone()
        {
            NamedResource::new(id, field.value.clone())
        } else {
            panic!(
                "block {:?} must have an identified field named {name:?}",
                self.opcode
            );
        }
    }

    pub fn var_field(&self, name: &str) -> VariableRef {
        self.identified_field(name).into()
    }

    pub fn try_as_primitive(&self) -> Option<Primitive> {
        Some(match &*self.opcode {
            Self::TEXT => Primitive::Text(self.simple_field(Self::TEXT_FIELD)),
            Self::NUMBER => Primitive::Number(self.parsed_field(Self::NUM_FIELD)),
            Self::INTEGER => Primitive::Integer(self.parsed_field(Self::NUM_FIELD)),
            Self::WHOLE_NUMBER => Primitive::WholeNumber(self.parsed_field(Self::NUM_FIELD)),
            Self::POSITIVE_NUMBER => {
                let pos_num: f64 = self.parsed_field(Self::NUM_FIELD);
                if pos_num.is_sign_negative() {
                    panic!("{pos_num:?} is not a valid positive number");
                }
                Primitive::PositiveNumber(pos_num)
            }
            Self::ANGLE => Primitive::Angle(self.parsed_field(Self::NUM_FIELD)),
            Self::VARIABLE => Primitive::Variable(self.identified_field(Self::VAR_FIELD).into()),
            Self::EVENT => Primitive::Event(self.identified_field(Self::EVENT_FIELD).into()),
            _ => return None,
        })
    }

    pub fn try_as_start_condition(&self) -> Option<StartCondition> {
        Some(match &*self.opcode {
            "event_whenflagclicked" => StartCondition::FlagClicked,
            "event_whenbroadcastreceived" => {
                let field = &self.fields[Self::EVENT_FIELD]
                    .try_to_named_resource()
                    .expect("BroadcastReceived block missing event id");

                StartCondition::BroadcastReceived(Event::from(field.clone()))
            }
            "procedures_definition" => {
                let custom_block = &self.inputs[Self::PROCECURE_DEFN_PROTOTYPE].unwrap_single_ref();

                let mut prototype = ProcedurePrototype::new(
                    custom_block
                        .proc_code
                        .clone()
                        .expect("procedure definition missing proc code"),
                );

                for (id, input) in &custom_block.inputs {
                    let reporter = input.unwrap_single_ref();
                    let value = reporter.simple_field(Self::ARG_NAME_FIELD);
                    let arg = ProcedureArgument::new(id.clone(), value);
                    prototype = prototype.with_arg(arg);
                }

                StartCondition::ProcedureCalled(prototype)
            }
            _ => return None,
        })
    }
}

impl From<VariableRef> for Block {
    fn from(value: VariableRef) -> Self {
        Self::var(value.0.id, value.0.name)
    }
}

impl From<Event> for Block {
    fn from(value: Event) -> Self {
        Self::event(value.0.id, value.0.name)
    }
}

#[derive(Debug, Unwrap, TryUnwrap)]
pub enum Primitive {
    Text(Arc<str>),
    Number(f64),
    Integer(u64),
    WholeNumber(i64),
    PositiveNumber(f64),
    Angle(f64),
    Variable(VariableRef),
    Event(Event),
}

#[derive(Debug)]
pub struct Input {
    pub blocks: Vec<Block>,
    pub shadow: Option<Block>,
}

impl Input {
    pub fn unwrap_single_ref(&self) -> &Block {
        assert!(self.blocks.len() == 1, "expected single block");
        &self.blocks[0]
    }
}

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
    pub value: Arc<str>,
    pub id: Option<Arc<str>>,
}

impl Field {
    pub fn simple(value: impl Into<Arc<str>>) -> Self {
        Self {
            value: value.into(),
            id: None,
        }
    }

    pub fn identified(id: impl Into<Arc<str>>, value: impl Into<Arc<str>>) -> Self {
        Self {
            value: value.into(),
            id: Some(id.into()),
        }
    }

    pub fn try_to_named_resource(&self) -> Option<NamedResource> {
        self.id.as_ref().map(|id| NamedResource {
            name: self.value.clone(),
            id: id.clone(),
        })
    }

    pub fn unwrap_variable(&self) -> VariableRef {
        let Some(resource) = self.try_to_named_resource() else {
            panic!("{self:?} must be representable as variable");
        };

        resource.into()
    }
}

impl<T: Into<NamedResource>> From<T> for Field {
    fn from(value: T) -> Self {
        let resource: NamedResource = value.into();
        Self::identified(resource.id, resource.name)
    }
}

#[derive(Debug, Clone, Constructor, PartialEq, Eq, Hash)]
pub struct NamedResource {
    pub id: Arc<str>,
    pub name: Arc<str>,
}

#[derive(Debug, Clone, AsRef)]
pub struct Variable {
    pub reference: VariableRef,
    pub initial_value: Value,
}

impl Variable {
    pub fn new(reference: VariableRef, initial_value: Value) -> Self {
        Self {
            reference,
            initial_value,
        }
    }

    pub fn empty(reference: VariableRef) -> Self {
        Self {
            reference,
            initial_value: Value::default(),
        }
    }

    pub fn id(&self) -> Arc<str> {
        self.reference.id()
    }

    pub fn name(&self) -> Arc<str> {
        self.reference.name()
    }
}

#[derive(Debug, Clone, From, Into, AsRef, PartialEq, Eq, Hash)]
pub struct VariableRef(NamedResource);

impl VariableRef {
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
        Self(NamedResource::new(id.into(), name.into()))
    }

    pub fn into_inner(self) -> NamedResource {
        self.0
    }

    pub fn id(&self) -> Arc<str> {
        self.0.id.clone()
    }

    pub fn name(&self) -> Arc<str> {
        self.0.name.clone()
    }
}

#[derive(Debug, Clone, From, Into, AsRef)]
pub struct Event(NamedResource);

impl Event {
    pub fn new(id: impl Into<Arc<str>>, name: impl Into<Arc<str>>) -> Self {
        Self(NamedResource::new(id.into(), name.into()))
    }

    pub fn into_inner(self) -> NamedResource {
        self.0
    }

    pub fn id(&self) -> Arc<str> {
        self.0.id.clone()
    }

    pub fn name(&self) -> Arc<str> {
        self.0.name.clone()
    }
}
