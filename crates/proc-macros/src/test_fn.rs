use proc_macro2::TokenStream;
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Attribute, FnArg, Ident, ReturnType, Token, Visibility, braced, parenthesized};

#[derive(Debug)]
pub struct TestFn {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub asyncness: Option<Token![async]>,
    pub ident: Ident,
    pub fn_args: Punctuated<FnArg, Token![,]>,
    pub return_type: ReturnType,
    pub fn_body: TokenStream,
}

impl Parse for TestFn {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        // let constness: Option<Token![const]> = input.parse()?;
        let asyncness: Option<Token![async]> = input.parse()?;
        // let unsafety: Option<Token![unsafe]> = input.parse()?;
        // let abi: Option<Abi> = input.parse()?;
        let _: Token![fn] = input.parse()?;
        let ident: Ident = input.parse()?;
        let fn_args_buf;
        parenthesized!(fn_args_buf in input);
        let fn_args = Punctuated::<FnArg, Token![,]>::parse_terminated(&fn_args_buf)?;
        let return_type: ReturnType = input.parse()?;
        let fn_body;
        braced!(fn_body in input);
        Ok(Self {
            attrs,
            vis,
            asyncness,
            ident,
            fn_args,
            return_type,
            fn_body: fn_body.parse()?,
        })
    }
}
