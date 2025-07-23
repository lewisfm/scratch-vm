use std::rc::Rc;

use derive_more::{Constructor, From, Into, Unwrap};

use crate::ast::NamedResource;

#[derive(Debug, From, Unwrap)]
pub enum Primitive {
    Text(Text),
    MathNumber(math::Number),
    MathInteger(math::Integer),
    MathWholeNumber(math::WholeNumber),
    MathPositiveNumber(math::PositiveNumber),
    MathAngle(math::Angle),
    DataVariable(data::Variable),
    DataList(data::ListContents),
    BroadcastMenu(event::BroadcastMenu),
}

#[derive(Debug, From, Into, Constructor)]
pub struct Text(pub Rc<str>);

pub mod math {
    use super::*;

    #[derive(Debug, From, Into, Constructor)]
    pub struct Number(pub f64);

    #[derive(Debug, From, Into, Constructor)]
    pub struct Integer(pub i64);

    #[derive(Debug, From, Into, Constructor)]
    pub struct WholeNumber(pub u64);

    #[derive(Debug, Into)]
    pub struct PositiveNumber(pub f64);

    impl PositiveNumber {
        pub fn new(num: f64) -> Option<Self> {
            (num >= 0.0).then_some(Self(num))
        }
    }

    #[derive(Debug, From, Into, Constructor)]
    pub struct Angle(pub f64);
}

pub mod data {
    use super::*;

    #[derive(Debug, From, Into, Constructor)]
    pub struct Variable(pub NamedResource);

    #[derive(Debug, From, Into, Constructor)]
    pub struct ListContents(pub NamedResource);
}

pub mod event {
    use super::*;

    #[derive(Debug, From, Into, Constructor)]
    pub struct BroadcastMenu(pub NamedResource);
}
