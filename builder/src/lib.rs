#[macro_use]
extern crate darling;

mod builder;

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive_builder(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    builder::derive(proc_macro2::TokenStream::from(input)).into()
}
