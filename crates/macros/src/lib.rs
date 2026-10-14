//! The derives behind Graticule apps: `Facts`, `Commands`, `Form`, `Choice` and `t!`.

mod commands;
mod facts;
mod form;
mod message;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;

/// Facts: an app's state, made of plain values and Ledger's types. Generates a path per field (`Todo::draft`), saving and loading, the comparison that turns changes into operations, and `Find` for every `List` field.
#[proc_macro_derive(Facts, attributes(key, session, live, resource, derived))]
pub fn derive_facts(input: TokenStream) -> TokenStream {
    facts::derive(syn::parse_macro_input!(input as syn::DeriveInput)).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Commands: everything that can happen in an app, with names, labels and subjects for the registry.
#[proc_macro_derive(Commands, attributes(label, subject, internal))]
pub fn derive_commands(input: TokenStream) -> TokenStream {
    commands::derive(syn::parse_macro_input!(input as syn::DeriveInput)).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Form: a struct of settings. Each field marked `#[label("…")]` is one setting, drawn by `ui.form` with a control chosen by its type and compared with the struct's `Default`.
#[proc_macro_derive(Form, attributes(label))]
pub fn derive_form(input: TokenStream) -> TokenStream {
    form::derive_form(syn::parse_macro_input!(input as syn::DeriveInput)).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Choice: an enum of named options a setting can take, each labelled by `#[label("…")]` or by its name in words.
#[proc_macro_derive(Choice, attributes(label))]
pub fn derive_choice(input: TokenStream) -> TokenStream {
    form::derive_choice(syn::parse_macro_input!(input as syn::DeriveInput)).unwrap_or_else(syn::Error::into_compile_error).into()
}

/// Text the user reads, written in English and marked for translation: `t!("Saved to {path}")`.
#[proc_macro]
pub fn t(input: TokenStream) -> TokenStream {
    message::expand(syn::parse_macro_input!(input as syn::LitStr)).unwrap_or_else(syn::Error::into_compile_error).into()
}

pub(crate) fn ledger() -> proc_macro2::TokenStream {
    if let Ok(FoundCrate::Name(name)) = crate_name("graticule") {
        let name = quote::format_ident!("{name}");
        return quote!(::#name::ledger);
    }
    match crate_name("ledger") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let name = quote::format_ident!("{name}");
            quote!(::#name)
        }
        Err(_) => quote!(::graticule::ledger),
    }
}

pub(crate) fn kernel() -> proc_macro2::TokenStream {
    if let Ok(FoundCrate::Name(name)) = crate_name("graticule") {
        let name = quote::format_ident!("{name}");
        return quote!(::#name::kernel);
    }
    match crate_name("kernel") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let name = quote::format_ident!("{name}");
            quote!(::#name)
        }
        Err(_) => quote!(::graticule::kernel),
    }
}
