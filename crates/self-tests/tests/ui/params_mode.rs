use testscribe::{ParamDisplay, testscribe};

#[derive(Debug, Clone, ParamDisplay)]
pub struct Param {}

#[testscribe(params, tags=[fasd])]
fn params() -> Vec<Param> {
    Default::default()
}

fn main () {}
