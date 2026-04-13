use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_quote};

pub fn expand_clone_async(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let mut generics = input.generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: ::core::clone::Clone));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        impl #impl_generics ::testscribe::clone_async::CloneAsync for #name #ty_generics #where_clause {
            async fn clone_async(&self) -> Self {
                ::core::clone::Clone::clone(self)
            }
        }
    };

    expanded.into()
}
