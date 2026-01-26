use std::{
    any::{Any, type_name},
    future::Future,
    pin::Pin,
};

use serde::{Serialize, Serializer};

use crate::{
    report::TestReport,
    test_args::{Env, Environment, Given, Param, ParamDisplay, Parameter, ParentTest},
    test_case::name::FqFnName,
};

pub struct Value(Box<dyn Any + 'static>);

impl Value {
    pub fn new<T: 'static>(value: T) -> Self {
        Self(Box::new(Some(value)))
    }
    pub fn take<T: 'static>(&mut self) -> T {
        let binding: &mut Option<T> = self.0.downcast_mut().unwrap();
        binding.take().unwrap()
    }
    pub fn as_mut_ref<T: 'static>(&mut self) -> &mut T {
        let env: &mut Option<T> = self.0.downcast_mut().unwrap();
        env.as_mut().unwrap()
    }
    pub fn clone_as<T: Clone + 'static>(&self) -> Self {
        let binding: &Option<T> = self.0.downcast_ref().unwrap();
        Self(Box::new(binding.clone()))
    }
}

type AsyncTestFn = for<'a> fn(
    TestReport,
    Value,
    &'a mut Value,
    Value,
) -> Pin<Box<dyn Future<Output = Value> + 'a>>;
type SyncTestFn = fn(TestReport, Value, &mut Value, Value) -> Value;

#[derive(Debug)]
pub enum TestFn {
    SyncFn(SyncTestFn),
    AsyncFn(AsyncTestFn),
}

impl Serialize for TestFn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            TestFn::SyncFn(_) => serializer.serialize_str("SyncFn"),
            TestFn::AsyncFn(_) => serializer.serialize_str("AsyncFn"),
        }
    }
}

impl TestFn {
    pub async fn invoke(
        &self,
        then: TestReport,
        state: Value,
        env: &mut Value,
        params: Value,
    ) -> Value {
        match self {
            TestFn::SyncFn(f) => f(then, state, env, params),
            TestFn::AsyncFn(f) => f(then, state, env, params).await,
        }
    }
    pub fn is_async(&self) -> bool {
        matches!(self, TestFn::AsyncFn(_))
    }
}

#[derive(Debug)]
pub struct ParentFn {
    pub get_name: fn() -> FqFnName<'static>,
}

impl ParentFn {
    pub const fn from_sync<P, E, A, StateOut>(
        _test_fn: fn(TestReport, Given<P>, Env<'_, E>, Param<A>) -> StateOut,
    ) -> ParentFn
    where
        P: ParentTest,
        E: Environment + 'static,
        A: Parameter,
    {
        ParentFn {
            get_name: || name_from_type::<P>(),
        }
    }

    pub const fn from_async<'a, P, E, A, StateOut, Fut>(
        _test_fn: fn(TestReport, Given<P>, Env<'a, E>, Param<A>) -> Fut,
    ) -> ParentFn
    where
        Fut: Future<Output = StateOut> + 'a,
        P: ParentTest,
        E: Environment + 'static,
        A: Parameter,
    {
        ParentFn {
            get_name: || name_from_type::<P>(),
        }
    }
}

pub struct TestParam {
    pub header: Vec<&'static str>,
    pub display_str: Vec<String>,
    pub value: Value,
}

pub struct TestParams {
    header: Vec<&'static str>,
    display_values: Vec<Vec<String>>,
    values: Box<dyn ParamsValues>,
}

impl TestParams {
    pub fn new<T>(iter: impl IntoIterator<Item = T>) -> Self
    where
        T: ParamDisplay + 'static,
    {
        let values: Vec<T> = iter.into_iter().collect();
        Self {
            header: Vec::from(T::NAMES),
            display_values: values.iter().map(|v| v.values()).collect(),
            values: Box::new(values),
        }
    }

    pub fn new_empty() -> Self {
        Self::new::<()>([])
    }

    pub fn len(&self) -> usize {
        self.display_values.len()
    }

    pub fn get(&self, index: usize) -> TestParam {
        TestParam {
            header: self.header.clone(),
            value: self.values.get(index),
            display_str: self.display_values[index].clone(),
        }
    }
}

trait ParamsValues {
    fn get(&self, index: usize) -> Value;
}

impl<T> ParamsValues for Vec<T>
where
    T: Clone + 'static,
{
    fn get(&self, index: usize) -> Value {
        Value::new(self[index].clone())
    }
}

#[derive(Debug)]
pub struct ParamsFn {
    pub params: fn() -> TestParams,
}

impl ParamsFn {
    pub const fn from_sync<P, E, A, StateOut>(
        _test_fn: fn(TestReport, Given<P>, Env<'_, E>, Param<A>) -> StateOut,
    ) -> ParamsFn
    where
        P: ParentTest,
        E: Environment + 'static,
        A: Parameter,
    {
        ParamsFn {
            params: || TestParams::new(A::create()),
        }
    }

    pub const fn from_async<'a, P, E, A, StateOut, Fut>(
        _test_fn: fn(TestReport, Given<P>, Env<'a, E>, Param<A>) -> Fut,
    ) -> ParamsFn
    where
        Fut: Future<Output = StateOut> + 'a,
        P: ParentTest,
        E: Environment + 'static,
        A: Parameter,
    {
        ParamsFn {
            params: || TestParams::new(A::create()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnvFns {
    pub push_env: fn(Value) -> Pin<Box<dyn Future<Output = Value>>>,
    pub get_name: fn() -> FqFnName<'static>,
    pub arg_type: fn() -> &'static str,
    pub return_type: fn() -> &'static str,
}

impl EnvFns {
    pub const fn from_sync<P, E, A, StateOut>(
        _test_fn: fn(TestReport, Given<P>, Env<'_, E>, Param<A>) -> StateOut,
    ) -> EnvFns
    where
        P: ParentTest,
        E: Environment + 'static,
        A: Parameter,
    {
        EnvFns {
            push_env: |mut parent| {
                let value = parent.take();
                Box::pin(async move { Value::new(E::push(value).await) })
            },
            get_name: || name_from_type::<E>(),
            arg_type: || type_name::<E::Parent>(),
            return_type: || type_name::<E::Current>(),
        }
    }

    pub const fn from_async<'a, P, E, A, StateOut, Fut>(
        _test_fn: fn(TestReport, Given<P>, Env<'a, E>, Param<A>) -> Fut,
    ) -> EnvFns
    where
        Fut: Future<Output = StateOut> + 'a,
        P: ParentTest,
        E: Environment + 'static,
        A: Parameter,
    {
        EnvFns {
            push_env: |mut parent| {
                Box::pin(async move { Value::new(E::push(parent.take()).await) })
            },
            get_name: || name_from_type::<E>(),
            arg_type: || type_name::<E::Parent>(),
            return_type: || type_name::<E::Current>(),
        }
    }
}

#[derive(Debug)]
pub struct CloneFns {
    pub state: fn(&Value) -> Value,
    pub env: fn(&Value) -> Value,
}

impl CloneFns {
    pub const fn from_sync<P, E, A, StateOut>(
        _test_fn: fn(TestReport, Given<P>, Env<'_, E>, Param<A>) -> StateOut,
    ) -> CloneFns
    where
        StateOut: Clone + Send + 'static,
        P: ParentTest,
        E: Environment,
        E::Current: Clone + 'static,
        A: Parameter,
    {
        CloneFns {
            state: |outcome| outcome.clone_as::<StateOut>(),
            env: |current| current.clone_as::<E::Current>(),
        }
    }

    pub const fn from_async<'a, P, E, A, StateOut, Fut>(
        _test_fn: fn(TestReport, Given<P>, Env<'a, E>, Param<A>) -> Fut,
    ) -> CloneFns
    where
        StateOut: Clone + Send + 'static,
        Fut: Future<Output = StateOut> + 'a,
        P: ParentTest,
        E: Environment,
        E::Current: Clone + 'static,
        A: Parameter,
    {
        CloneFns {
            state: |outcome| outcome.clone_as::<StateOut>(),
            env: |current| current.clone_as::<E::Current>(),
        }
    }
}

fn name_from_type<T>() -> FqFnName<'static> {
    let name = type_name::<T>();
    let (path, name) = name.rsplit_once("::").unwrap();
    FqFnName { path, name }
}
