mod fns;

mod name;

use serde::Serialize;

pub use fns::{CloneFns, EnvFns, ParamsFn, ParentFn, TestFn, TestParams, Value};
pub use name::FqFnName;

#[derive(Debug, Serialize)]
pub struct TestCase {
    pub name: FqFnName<'static>,
    pub tags: &'static [&'static str],
    pub filename: &'static str,
    pub line_nr: u32,
    pub test_fn: TestFn,
    #[serde(skip_serializing)]
    pub clone: Option<CloneFns>,
    #[serde(skip_serializing)]
    pub parent: Option<ParentFn>,
    #[serde(skip_serializing)]
    pub env: Option<EnvFns>,
    #[serde(skip_serializing)]
    pub params: Option<ParamsFn>,
}
