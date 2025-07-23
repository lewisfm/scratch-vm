use std::{collections::HashMap, rc::Rc};

use crate::interpreter::value::Value;

pub struct Block {
    pub opcode: Rc<str>,
    pub children: HashMap<Rc<str>, Child>,
    pub fields: HashMap<Rc<str>, Field>,
}

pub struct Child {
    pub block: Box<Block>,
    pub obscured_value: Option<Value>,
}

pub enum ChildValue {
    Reference(Rc<str>),
    Number(f64),
    String(Rc<str>),
    Event(Event),
}

pub struct Field {
    id: Option<Rc<str>>,
    value: Rc<str>,
}

impl Field {
    pub fn to_event(self) -> Option<Event> {
        self.id.map(|id| Event {
            name: self.value,
            id,
        })
    }
}

pub struct Event {
    name: Rc<str>,
    id: Rc<str>,
}
