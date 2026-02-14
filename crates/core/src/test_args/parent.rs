use std::ops::{Deref, DerefMut};

pub trait ParentTest {
    type Value;
}

impl ParentTest for () {
    type Value = ();
}

pub struct Given<P>(pub P::Value)
where
    P: ParentTest;

impl<P> Given<P>
where
    P: ParentTest,
{
    pub fn into_inner(self) -> P::Value {
        self.0
    }
}

impl<P> Deref for Given<P>
where
    P: ParentTest,
{
    type Target = P::Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P> DerefMut for Given<P>
where
    P: ParentTest,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
