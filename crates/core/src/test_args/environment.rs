use std::future::Future;
use std::ops::{Deref, DerefMut};

pub trait Environment {
    type Base: Environment;
    fn create(base: Self::Base) -> impl Future<Output = Self>;
}

impl Environment for () {
    type Base = ();
    fn create(_base: Self::Base) -> impl Future<Output = Self> {
        async {}
    }
}

pub struct Env<'a, E>(pub &'a mut E)
where
    E: Environment;

impl<E> Deref for Env<'_, E>
where
    E: Environment,
{
    type Target = E;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<E> DerefMut for Env<'_, E>
where
    E: Environment,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}
