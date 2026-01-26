use std::future::Future;
use std::ops::{Deref, DerefMut};

pub trait Environment {
    type Parent;
    type Current;
    fn push(parent: Self::Parent) -> impl Future<Output = Self::Current>;
}

impl Environment for () {
    type Parent = ();
    type Current = ();
    fn push(_parent: Self::Parent) -> impl Future<Output = Self::Current> {
        async {}
    }
}

pub struct Env<'a, E>(pub &'a mut E::Current)
where
    E: Environment;

impl<E> Deref for Env<'_, E>
where
    E: Environment,
{
    type Target = E::Current;

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
