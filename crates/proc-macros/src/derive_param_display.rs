use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

pub fn expand_param_display(input: DeriveInput) -> TokenStream {
    let struct_name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ParamDisplay only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "ParamDisplay can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let mut field_name_strings = Vec::new();
    let mut field_name_exprs = Vec::new();
    for f in fields {
        let ident = f.ident.as_ref().unwrap();
        field_name_strings.push(ident.to_string());
        let mut expr = None;
        for attr in &f.attrs {
            if attr.path().is_ident("pd") {
                let mut is_debug = false;
                let mut is_custom = None;
                if let Err(err) = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("debug") {
                        is_debug = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("custom") {
                        let value: syn::Path = meta.value()?.parse()?;
                        is_custom = Some(value);
                        return Ok(());
                    }
                    Err(meta.error("valid options: `debug`, `custom=my_display_fn`"))
                }) {
                    return err.to_compile_error().into();
                }
                if is_debug {
                    expr = Some(quote!(format!("{:?}", self.#ident)))
                } else if let Some(display_fn) = is_custom {
                    expr = Some(quote!( #display_fn(&self.#ident)))
                }
            }
        }
        field_name_exprs.push(expr.unwrap_or_else(|| quote!( self.#ident.to_string() )))
    }

    let expanded = quote! {
        impl ::testscribe::test_args::ParamDisplay for #struct_name {
            const NAMES: &'static [&'static str] = &[
                #( #field_name_strings ),*
            ];

            fn values(&self) -> ::std::vec::Vec<::std::string::String> {
                vec![
                    #( #field_name_exprs ),*
                ]
            }
        }
    };

    expanded.into()
}
