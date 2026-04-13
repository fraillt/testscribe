use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Error, FnArg, GenericArgument, Ident, Lifetime, PathArguments, Type};

use crate::config::TestConfig;
use crate::test_fn::TestFn;
use crate::utils::snake_to_pascal;

pub fn expand_test(config: TestConfig, test_fn: TestFn) -> TokenStream {
    let TestFn {
        attrs,
        vis,
        asyncness,
        mut ident,
        fn_args,
        return_type,
        fn_body,
    } = test_fn;
    let test_case = Ident::new(
        &format!("TEST_CASE_{}", ident.to_string().to_ascii_uppercase()),
        ident.span(),
    );
    let visible_name = ident.to_string();
    let visible_ident = ident.clone();
    let test_struct_name = snake_to_pascal(&visible_name);
    let test_struct = Ident::new(&test_struct_name, visible_ident.span());
    // `#[cfg]` must gate every generated item (registration static, marker type, wrapper),
    // not only the test function itself.
    let cfg_attrs: Vec<_> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .collect();
    let standalone = if let Some(standalone) = &config.standalone {
        // Harness attributes belong to the generated wrapper. On the function itself they
        // would apply to the test body, which is not a valid harness entry point.
        for attr in &attrs {
            if let Some(segment) = attr.path().segments.last()
                && segment.ident == "test"
            {
                return Error::new(
                    attr.span(),
                    "test harness attributes belong to the generated wrapper; pass them inside \
                     `standalone(...)` instead, e.g. `#[testscribe(standalone(tokio::test))]`",
                )
                .to_compile_error();
            }
        }
        let harness = if standalone.harness.is_empty() {
            if asyncness.is_some() {
                return Error::new(
                    standalone.path.span(),
                    "async standalone test requires a harness attribute, e.g. `standalone(tokio::test)`",
                )
                .to_compile_error();
            }
            vec![quote!(#[test])]
        } else {
            standalone
                .harness
                .iter()
                .map(|meta| quote!(#[#meta]))
                .collect()
        };
        ident = Ident::new(
            &format!("standalone_{}", visible_ident),
            visible_ident.span(),
        );
        Some(if asyncness.is_some() {
            quote! {
                #(#cfg_attrs)*
                #(#harness)*
                async fn #visible_ident() -> Result<(), String> {
                    testscribe::standalone::run_async(&testscribe::CASES, module_path!(), #test_struct_name).await
                }
            }
        } else {
            quote! {
                #(#cfg_attrs)*
                #(#harness)*
                fn #visible_ident() -> Result<(), String> {
                    testscribe::standalone::run_sync(&testscribe::CASES, module_path!(), #test_struct_name)
                }
            }
        })
    } else {
        None
    };

    let fq_name = quote!(testscribe::test_case::FqFnName::new(module_path!(), #test_struct_name));
    let tags = config.tags.iter().map(|tag| tag.to_string());
    let test_fn = if asyncness.is_some() {
        quote!(testscribe::test_case::TestFn::AsyncFn(|ctx, mut parent, mut env, mut param| Box::pin(async move {
            testscribe::test_case::Value::new(#ident(ctx,
                testscribe::test_args::Given(parent.take()),
                testscribe::test_args::Env(env.as_mut()),
                testscribe::test_args::Param(param.take())
            ).await)
        })))
    } else {
        quote!(testscribe::test_case::TestFn::SyncFn(|ctx, mut parent, mut env, mut param| {
            testscribe::test_case::Value::new(#ident(ctx,
                testscribe::test_args::Given(parent.take()),
                testscribe::test_args::Env(env.as_mut()),
                testscribe::test_args::Param(param.take())))
        }))
    };
    let clone_fns = if config.is_cloneable.is_some() {
        if asyncness.is_some() {
            quote!(Some(testscribe::test_case::CloneFns::from_async_clone_sync(#ident)))
        } else {
            quote!(Some(testscribe::test_case::CloneFns::from_sync_clone_sync(#ident)))
        }
    } else if let Some(cloneable_async) = config.is_cloneable_async {
        if asyncness.is_some() {
            quote!(Some(testscribe::test_case::CloneFns::from_async_clone_async(#ident)))
        } else {
            return Error::new(
                cloneable_async.span(),
                "Cannot use `cloneable_async` when test function is sync",
            )
            .to_compile_error();
        }
    } else {
        quote!(None)
    };
    let is_async = if asyncness.is_some() {
        quote!(true)
    } else {
        quote!(false)
    };
    let mut arg_parent = None;
    let mut arg_env = None;
    let mut arg_param = None;
    for arg in fn_args.into_iter() {
        if let FnArg::Typed(ref t) = arg {
            if let Type::Path(p) = t.ty.as_ref()
                && let Some(segment) = p.path.segments.last()
                && let PathArguments::AngleBracketed(_) = segment.arguments
            {
                if segment.ident == "Param" {
                    if let Some(already_exists) = arg_param.replace(quote!( #arg )) {
                        return Error::new(already_exists.span(), "`Param` already defined here")
                            .to_compile_error();
                    };
                    continue;
                }
                if segment.ident == "Env" {
                    // Normalize `Env<E>` to `Env<'_, E>`. The generated wrapper is always a real
                    // `fn`, and async fns reject the elided form (E0726), so without this users
                    // would have to spell out the lifetime themselves in every async test.
                    let mut env_arg = arg.clone();
                    insert_anonymous_lifetime(&mut env_arg);
                    if let Some(already_exists) = arg_env.replace(quote!( #env_arg )) {
                        return Error::new(already_exists.span(), "`Env` already defined here")
                            .to_compile_error();
                    };
                    continue;
                }
                if segment.ident == "Given" {
                    if let Some(already_exists) = arg_parent.replace(quote!( #arg )) {
                        return Error::new(already_exists.span(), "`Given` already defined here")
                            .to_compile_error();
                    };
                    continue;
                }
            }
            return Error::new(
                arg.span(),
                "Test arguments must be wrapped in one of: Given, Env, Param",
            )
            .to_compile_error();
        } else {
            return Error::new(arg.span(), "Self is not allowed here").to_compile_error();
        }
    }

    let parent_fn = if arg_parent.is_some() {
        if asyncness.is_some() {
            quote!(Some(testscribe::test_case::ParentFn::from_async(#ident)))
        } else {
            quote!(Some(testscribe::test_case::ParentFn::from_sync(#ident)))
        }
    } else {
        quote!(None)
    };

    let env_fns = if arg_env.is_some() {
        if asyncness.is_some() {
            quote!(Some(testscribe::test_case::EnvFns::from_async(#ident)))
        } else {
            quote!(Some(testscribe::test_case::EnvFns::from_sync(#ident)))
        }
    } else {
        quote!(None)
    };

    let params_fn = if arg_param.is_some() {
        if asyncness.is_some() {
            quote!(Some(testscribe::test_case::ParamsFn::from_async(#ident)))
        } else {
            quote!(Some(testscribe::test_case::ParamsFn::from_sync(#ident)))
        }
    } else {
        quote!(None)
    };
    if arg_parent.is_some()
        && let Some(standalone) = &config.standalone
    {
        return Error::new(
            standalone.path.span(),
            "Standalone test must be a root test (no parent)",
        )
        .to_compile_error();
    }

    let arg_parent = arg_parent.unwrap_or_else(|| quote!( _: testscribe::test_args::Given<()>));
    let arg_env = arg_env.unwrap_or_else(|| quote!( _: testscribe::test_args::Env<'_, ()>));
    let arg_param = arg_param.unwrap_or_else(|| quote!( _: testscribe::test_args::Param<()>));

    let mut res = quote! {
        #(#cfg_attrs)*
        #[testscribe::linkme::distributed_slice(testscribe::CASES)]
        #[linkme(crate = testscribe::linkme)]
        static #test_case: testscribe::test_case::TestCase = testscribe::test_case::TestCase {
            name: #fq_name,
            tags: &[#(#tags,)*],
            filename: file!(),
            line_nr: line!(),
            test_fn: #test_fn,
            parent: #parent_fn,
            env: #env_fns,
            params: #params_fn,
            clone: #clone_fns
        };
    };

    let return_type_only = match &return_type {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, t) => quote!( #t),
    };
    let return_type_str = return_type_only.to_string();
    res.extend(quote! {
        #(#cfg_attrs)*
        #[doc = concat!("Returns [", #return_type_str, "]")]
        #vis struct #test_struct;
        #(#cfg_attrs)*
        impl testscribe::test_args::ParentTest for #test_struct {
            type Value = #return_type_only;
        }
    });

    res.extend(quote! {
        #(#attrs)*
        #vis #asyncness fn #ident(mut _report: testscribe::report::TestReport, #arg_parent, #arg_env, #arg_param) #return_type {
            macro_rules! then {
                ($var:ident) => {
                    ::testscribe::report::VerifyValue::new(
                        &mut _report,
                        &($var),
                        stringify!($var),
                        line!(),
                        file!(),
                    )
                };
                ($var:expr => $as_var:ident) => {
                    ::testscribe::report::VerifyValue::new(&mut _report, &($var), stringify!($as_var), line!(), file!())
                };
                ($msg:literal) => {
                    ::testscribe::report::VerifyStatement::<#is_async>::new(&mut _report, $msg, line!(), file!())
                };
            }

            #fn_body
        }
        #standalone
    });

    res
}

/// Ensures an `Env<...>` argument's first generic is a lifetime, inserting an anonymous `'_`
/// when the user omitted it. This lets `Env<E>` be written uniformly in both sync and async
/// tests; an already-present lifetime (e.g. `Env<'_, E>`) is left untouched.
fn insert_anonymous_lifetime(arg: &mut FnArg) {
    if let FnArg::Typed(typed) = arg
        && let Type::Path(path) = typed.ty.as_mut()
        && let Some(segment) = path.path.segments.last_mut()
        && let PathArguments::AngleBracketed(generics) = &mut segment.arguments
        && !matches!(generics.args.first(), Some(GenericArgument::Lifetime(_)))
    {
        generics.args.insert(
            0,
            GenericArgument::Lifetime(Lifetime::new("'_", segment.ident.span())),
        );
    }
}
