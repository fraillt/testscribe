use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{Error, Ident, PathArguments, ReturnType, Type};

use crate::test_fn::TestFn;
use crate::utils::snake_to_pascal;

pub fn expand_params(test_fn: TestFn) -> TokenStream {
    let TestFn {
        attrs,
        vis,
        asyncness,
        ident,
        fn_args,
        return_type,
        fn_body,
    } = test_fn;
    let visible_name = ident.to_string();
    let visible_ident = ident.clone();
    let test_struct_name = snake_to_pascal(&visible_name);
    let test_struct = Ident::new(&test_struct_name, visible_ident.span());

    if !fn_args.is_empty() {
        return Error::new(
            asyncness.span(),
            "arguments creation doesn't accept any arguments",
        )
        .to_compile_error();
    }
    if asyncness.is_some() {
        return Error::new(asyncness.span(), "Async arguments creation not implemented")
            .to_compile_error();
    }

    let mut return_inner_t = None;
    if let ReturnType::Type(_, ref t) = return_type {
        if let Type::Path(p) = t.as_ref() {
            if let Some(segment) = p.path.segments.last() {
                if let PathArguments::AngleBracketed(ref b) = segment.arguments {
                    let args = &b.args;
                    if segment.ident == "Vec" {
                        return_inner_t = Some(args);
                    }
                }
            }
        }
    }
    if return_inner_t.is_none() {
        return Error::new(return_type.span(), "Return type must be `Vec<T>`").to_compile_error();
    }
    let return_type_str = return_inner_t.unwrap().to_token_stream().to_string();
    quote! {
        #(#attrs)*
        #vis #asyncness fn #ident() #return_type {
            #fn_body
        }

        #[doc = concat!("Returns [", #return_type_str, "]")]
        #vis struct #test_struct;
        impl testscribe::test_args::Parameter for #test_struct {
            type Value = #return_inner_t;
            fn create() -> Vec<Self::Value> {
                #ident()
            }
        }
    }
}
