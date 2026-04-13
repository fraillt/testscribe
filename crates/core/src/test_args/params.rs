use std::ops::{Deref, DerefMut};

/// Controls how a parameter value is rendered as a table row in test output.
///
/// Usually implemented with `#[derive(ParamDisplay)]` from the `testscribe` crate, which uses
/// field names as column headers and renders each field with its `Display` implementation
/// (customizable per field via the `#[pd(debug)]` and `#[pd(custom = ...)]` attributes).
///
/// Required by parameter providers (`#[testscribe(params)]`) and by table-style checks
/// (`then!("...").params(list)`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `ParamDisplay`",
    note = "derive it: `#[derive(Clone, ParamDisplay)]` (renders each field as a table column)"
)]
pub trait ParamDisplay: Clone {
    /// Column headers, one per rendered field.
    const NAMES: &'static [&'static str];
    /// A single table row; must have the same length and order as [`NAMES`](ParamDisplay::NAMES).
    fn values(&self) -> Vec<String>;
}

impl ParamDisplay for () {
    const NAMES: &'static [&'static str] = &[];

    fn values(&self) -> Vec<String> {
        Default::default()
    }
}

/// Marks a type as a provider of test parameters.
///
/// Implemented automatically by `#[testscribe(params)]` for the marker type it generates
/// (the PascalCase version of the annotated function name). Tests reference the provider
/// through the [`Param`] argument.
///
/// You should never need to implement this trait manually.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a parameter provider",
    label = "expected the marker type of a `#[testscribe(params)]` provider",
    note = "`Param<P>` takes the PascalCase marker type that `#[testscribe(params)]` generates from the provider function's name (e.g. `fn test_params` -> `Param<TestParams>`)"
)]
pub trait Parameter {
    /// The parameter type produced by the provider.
    type Value: ParamDisplay + 'static;
    /// Returns the list of parameters; the referencing test runs once per item.
    fn create() -> Vec<Self::Value>;
}

impl Parameter for () {
    type Value = ();

    fn create() -> Vec<Self::Value> {
        Vec::default()
    }
}

/// Test argument that receives one parameter from a `#[testscribe(params)]` provider.
///
/// Declaring a `Param<Provider>` argument makes the test — together with its whole
/// subtree — run once for each parameter returned by the provider. The current parameter
/// is shown in the test output under the test name (e.g. `With nr=1`).
///
/// The inner value can be accessed by dereferencing (`p.nr`, via [`Deref`]/[`DerefMut`])
/// or by destructuring in the argument position: `Param(p): Param<Provider>`.
///
/// ```ignore
/// #[derive(Clone, ParamDisplay)]
/// struct TestCaseParam {
///     nr: i32,
/// }
///
/// #[testscribe(params)]
/// fn test_params() -> Vec<TestCaseParam> {
///     vec![TestCaseParam { nr: 1 }, TestCaseParam { nr: 2 }]
/// }
///
/// /// Runs once for nr=1 and once for nr=2.
/// #[testscribe(standalone)]
/// #[test]
/// fn add_numbers(p: Param<TestParams>) -> i32 {
///     then!(p.nr => nr).eq(p.nr);
///     p.nr
/// }
/// ```
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
