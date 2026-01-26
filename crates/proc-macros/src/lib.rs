mod config;
mod derive_param_display;
mod expand_env;
mod expand_params;
mod expand_test;
mod test_fn;
mod utils;

use config::Config;
use expand_env::expand_env;
use expand_params::expand_params;
use expand_test::expand_test;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};
use test_fn::TestFn;

use crate::derive_param_display::expand_param_display;

#[proc_macro_attribute]
pub fn testscribe(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = parse_macro_input!(attr as Config);
    let test_fn = parse_macro_input!(item as TestFn);
    match config {
        Config::Test(test_config) => TokenStream::from(expand_test(test_config, test_fn)),
        Config::Params => TokenStream::from(expand_params(test_fn)),
        Config::Environment => TokenStream::from(expand_env(test_fn)),
    }
}

#[proc_macro_derive(ParamDisplay, attributes(pd))]
pub fn param_display(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_param_display(input)
}
