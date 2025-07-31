use std::{
    borrow::Cow,
    cell::{OnceCell, RefCell},
    rc::Rc, sync::Arc,
};

use derive_more::{AsRef, From, Unwrap};

use crate::{ast::Variable, interpreter::id::Id};

#[derive(Debug, Clone, Unwrap, From, PartialEq)]
pub enum Value {
    String(Arc<str>),
    Number(f64),
    Boolean(bool),
    ReturnLocation(usize),
    Event(Id<EventValue>),
    Procedure(Id<ProcedureValue>),
}

impl Value {
    pub fn cast_string(&self) -> Arc<str> {
        match self {
            Value::String(string) => string.clone(),
            &Value::Number(num) => num.to_string().into(),
            &Value::Boolean(bool) => if bool { "true" } else { "false" }.into(),
            val => unimplemented!("cast {val:?} => string"),
        }
    }

    pub fn cast_number(&self) -> f64 {
        match self {
            &Value::Number(num) => num,
            Value::String(string) => string.parse().unwrap_or(0.0),
            &Value::Boolean(bool) => bool.into(),
            val => unimplemented!("cast {val:?} => number"),
        }
    }

    pub fn cast_boolean(&self) -> bool {
        match self {
            &Value::Boolean(bool) => bool,
            Value::String(string) => !string.is_empty(),
            &Value::Number(num) => num != 0.0,
            val => unimplemented!("cast {val:?} => boolean"),
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

impl Default for Value {
    fn default() -> Self {
        Self::String("".into())
    }
}

pub struct ProcedureValue {
    name: Option<Arc<str>>,
    pub(crate) param_count: usize,
    pub(crate) locals: Box<[Local]>,
    bytecode: Box<[u32]>,
    pub(super) ident: OnceCell<Id<Self>>,
}

impl ProcedureValue {
    pub fn new(
        name: Option<Arc<str>>,
        param_count: usize,
        locals: Box<[Local]>,
        instructions: Box<[u32]>,
    ) -> Self {
        if param_count > locals.len() {
            panic!("Too many params to store in the declared locals");
        }

        Self {
            name,
            param_count,
            locals,
            bytecode: instructions,
            ident: OnceCell::new(),
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("{unnamed}")
    }

    pub fn id(&self) -> Id<Self> {
        *self.ident.get().unwrap()
    }

    pub const fn bytecode(&self) -> &[u32] {
        &self.bytecode
    }

    pub fn as_value(&self) -> Value {
        Value::Procedure(self.id())
    }
}

#[derive(Debug, Clone)]
pub struct EventValue {
    name: Arc<str>,
}

impl EventValue {
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct VarState {
    pub name: Arc<str>,
    pub value: RefCell<Value>,
}

impl VarState {
    pub fn new(var: Variable) -> Self {
        Self {
            name: var.reference.name(),
            value: var.initial_value.into(),
        }
    }
}

impl AsRef<RefCell<Value>> for VarState {
    fn as_ref(&self) -> &RefCell<Value> {
        &self.value
    }
}

pub struct Local {
    name: Option<Arc<str>>,
}

impl Local {
    pub fn new(name: Option<Arc<str>>) -> Self {
        Self { name }
    }
}

impl From<&str> for Local {
    fn from(value: &str) -> Self {
        Self {
            name: Some(value.into()),
        }
    }
}
