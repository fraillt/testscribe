// re-export linkme
pub use linkme;
// re-export macros
pub use testscribe_proc_macros::ParamDisplay;
pub use testscribe_proc_macros::testscribe;

pub use testscribe_core::*;

#[cfg(feature = "standalone")]
pub mod standalone {
    pub use testscribe_standalone::*;
}

#[cfg(feature = "detached")]
pub mod detached {
    pub use testscribe_detached::*;
}

#[linkme::distributed_slice]
pub static CASES: [testscribe_core::test_case::TestCase];
