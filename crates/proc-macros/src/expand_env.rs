use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Error, FnArg, Ident, PatType};

use crate::test_fn::TestFn;
use crate::utils::snake_to_pascal;

pub fn expand_env(test_fn: TestFn) -> TokenStream {
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

    if fn_args.len() > 1 {
        return Error::new(
            fn_args.span(),
            "environment creation must accept parent environment as argument, or empty if parent is union type",
        )
        .to_compile_error();
    }
    let (modified_arg, modified_param, parent_type) = if let Some(arg) = fn_args.first() {
        let FnArg::Typed(PatType { ty, .. }) = arg else {
            return Error::new(fn_args.span(), "Cannot use self as an argument").to_compile_error();
        };
        (quote!( parent: #ty ), quote!(parent), quote!( #ty ))
    } else {
        (quote!( _: ()), quote!(), quote!(()))
    };

    let call = if asyncness.is_some() {
        quote!( #ident(#modified_param).await )
    } else {
        quote!( #ident(#modified_param))
    };

    let return_type_only = match &return_type {
        syn::ReturnType::Default => {
            return Error::new(return_type.span(), "environment must return something")
                .to_compile_error();
        }
        syn::ReturnType::Type(_, t) => quote!( #t),
    };

    let return_type_str = return_type_only.to_string();
    quote! {
        #(#attrs)*
        #vis #asyncness fn #ident(#fn_args) #return_type {
            #fn_body
        }

        #[doc = concat!("Returns [", #return_type_str, "]")]
        #vis struct #test_struct;
        impl testscribe::test_args::Environment for #test_struct {
            type Parent = #parent_type;
            type Current = #return_type_only;
            fn push(#modified_arg) -> impl ::std::future::Future<Output =Self::Current> {
                async move { #call }
            }
        }
    }
}
