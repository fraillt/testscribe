use std::fmt::Display;

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FqFnName<'a> {
    pub path: &'a str,
    pub name: &'a str,
}

impl<'a> FqFnName<'a> {
    pub const fn new(path: &'a str, name: &'a str) -> Self {
        Self { path, name }
    }
}

impl Display for FqFnName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}::{}", self.path, self.name)
    }
}
