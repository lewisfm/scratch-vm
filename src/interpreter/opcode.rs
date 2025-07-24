use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::interpreter::{id::Id, value::EventValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum Opcode {
    DoNothing,

    PushVar,
    SetVar,
    DecVar,
    ZeroVar,
    ClearVar,

    PushLocal,
    SetLocal,
    DecLocal,
    ZeroLocal,
    ClearLocal,

    PushZero,
    PushConstant,
    PushUInt32,
    PushNumber,

    Add,

    GreaterThan,

    DispatchEvent,
    CallBuiltin,
    CallProcedure,
    Jump,
    JumpIfTrue,
    JumpIfFalse,
    Return,
    Yield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
pub enum BuiltinProcedure {
    Say,
    LengthOf,
    LetterOf,
    Join,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Trigger {
    OnStart,
    Event(Id<EventValue>),
}
