use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr};

fn label(attrs: &[syn::Attribute]) -> syn::Result<Option<LitStr>> {
    attrs.iter().find(|a| a.path().is_ident("label")).map(|a| a.parse_args::<LitStr>()).transpose()
}

pub fn derive_form(input: DeriveInput) -> syn::Result<TokenStream> {
    let (kernel, ledger) = (crate::kernel(), crate::ledger());
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "a form is a struct whose labelled fields are its settings"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(name, "a form has named fields"));
    };
    let mut items = Vec::new();
    for f in &fields.named {
        let Some(label) = label(&f.attrs)? else { continue };
        let (ident, ty) = (f.ident.as_ref().expect("named"), &f.ty);
        items.push(quote! {
            out.push(#kernel::form::Item {
                label: #label,
                path: Self::#ident.erased(),
                value: #ledger::Field::value(&self.#ident),
                default: #ledger::Field::value(&defaults.#ident),
                control: <#ty as #kernel::Setting>::control(),
            });
        });
    }
    if items.is_empty() {
        return Err(syn::Error::new_spanned(name, "a form shows only fields marked #[label(\"…\")], and this one has none"));
    }
    Ok(quote! {
        impl #kernel::Settings for #name {
            fn items(&self, out: &mut ::std::vec::Vec<#kernel::form::Item>) {
                let defaults = <Self as ::std::default::Default>::default();
                #(#items)*
            }
        }
    })
}

pub fn derive_choice(input: DeriveInput) -> syn::Result<TokenStream> {
    let (kernel, ledger) = (crate::kernel(), crate::ledger());
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "a choice is an enum of named options"));
    };
    let mut options = Vec::new();
    for v in &data.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(&v.ident, "each option of a choice is a plain name, with nothing inside"));
        }
        let vi = &v.ident;
        let text = match label(&v.attrs)? {
            Some(l) => l.value(),
            None => words(&vi.to_string()),
        };
        options.push(quote!((#text, #ledger::Field::value(&Self::#vi))));
    }
    Ok(quote! {
        impl #kernel::Setting for #name {
            fn control() -> #kernel::Control {
                #kernel::Control::Choice(::std::vec![#(#options),*])
            }
        }
    })
}

fn words(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
