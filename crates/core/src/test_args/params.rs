use std::ops::{Deref, DerefMut};

pub trait ParamDisplay: Clone {
    const NAMES: &'static [&'static str];
    fn values(&self) -> Vec<String>;
}

impl ParamDisplay for () {
    const NAMES: &'static [&'static str] = &[];

    fn values(&self) -> Vec<String> {
        Default::default()
    }
}

pub trait Parameter {
    type Value: ParamDisplay + 'static;
    fn create() -> Vec<Self::Value>;
}

impl Parameter for () {
    type Value = ();

    fn create() -> Vec<Self::Value> {
        Vec::default()
    }
}

pub struct Param<A>(pub A::Value)
where
    A: Parameter;

impl<A> Deref for Param<A>
where
    A: Parameter,
{
    type Target = A::Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<A> DerefMut for Param<A>
where
    A: Parameter,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
