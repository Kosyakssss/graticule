use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Type};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Document,
    Session,
    Live,
    Resource,
}

struct Field {
    ident: syn::Ident,
    ty: Type,
    kind: Kind,
    key: bool,
}

fn kind_of(attrs: &[syn::Attribute]) -> Kind {
    for attr in attrs {
        let p = attr.path();
        if p.is_ident("session") {
            return Kind::Session;
        }
        if p.is_ident("live") || p.is_ident("derived") {
            return Kind::Live;
        }
        if p.is_ident("resource") {
            return Kind::Resource;
        }
    }
    Kind::Document
}

fn last_segment(ty: &Type) -> Option<&syn::PathSegment> {
    match ty {
        Type::Path(p) if p.qself.is_none() => p.path.segments.last(),
        _ => None,
    }
}

fn is_key(ty: &Type) -> bool {
    last_segment(ty).is_some_and(|s| s.ident == "Key" && !s.arguments.is_empty())
}

fn is_bool(ty: &Type) -> bool {
    last_segment(ty).is_some_and(|s| s.ident == "bool" && s.arguments.is_empty())
}

fn list_item(ty: &Type) -> Option<&Type> {
    let seg = last_segment(ty)?;
    if seg.ident != "List" {
        return None;
    }
    match &seg.arguments {
        syn::PathArguments::AngleBracketed(a) if a.args.len() == 1 => match &a.args[0] {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        },
        _ => None,
    }
}

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    match &input.data {
        Data::Struct(s) => derive_struct(&input, &s.fields),
        Data::Enum(e) => derive_enum(&input, e),
        Data::Union(_) => Err(syn::Error::new_spanned(&input.ident, "facts are a struct or an enum")),
    }
}

fn derive_struct(input: &DeriveInput, fields: &Fields) -> syn::Result<TokenStream> {
    let ledger = crate::ledger();
    let name = &input.ident;
    let Fields::Named(named) = fields else {
        return Err(syn::Error::new_spanned(name, "facts have named fields, so each one has a path"));
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(&input.generics, "facts can't be generic yet"));
    }
    let fields: Vec<Field> =
        named.named.iter().map(|f| Field { ident: f.ident.clone().expect("named"), ty: f.ty.clone(), kind: kind_of(&f.attrs), key: f.attrs.iter().any(|a| a.path().is_ident("key")) }).collect();
    let keys: Vec<&Field> = fields.iter().filter(|f| f.key).collect();
    if keys.len() > 1 {
        return Err(syn::Error::new_spanned(&keys[1].ident, "a record has one #[key]"));
    }
    let record = keys.first().copied();

    let kind_tokens = |k: Kind| match k {
        Kind::Document => quote!(#ledger::Kind::Document),
        Kind::Session => quote!(#ledger::Kind::Session),
        Kind::Live => quote!(#ledger::Kind::Live),
        Kind::Resource => quote!(#ledger::Kind::Resource),
    };

    let mut flag = 0u8;
    let mut flag_bits = Vec::new();
    let mut links = Vec::new();
    let mut consts = Vec::new();
    let mut infos = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        let (ident, ty) = (&f.ident, &f.ty);
        let index = i as u16;
        let label = ident.to_string();
        let kind = kind_tokens(f.kind);
        let flag_tokens = if record.is_some() && is_bool(ty) && f.kind != Kind::Live {
            let bit = flag;
            flag_bits.push(quote!((self.#ident as u64) << #bit));
            flag += 1;
            quote!(::std::option::Option::Some(#bit))
        } else {
            quote!(::std::option::Option::None)
        };
        let is_record_key = record.is_some_and(|k| k.ident == *ident);
        let link_tokens = if record.is_some() && !is_record_key && is_key(ty) && f.kind != Kind::Live {
            let n = links.len() as u8;
            links.push(ident.clone());
            quote!(::std::option::Option::Some(#n))
        } else {
            quote!(::std::option::Option::None)
        };
        consts.push(quote! {
            #[allow(non_upper_case_globals, dead_code)]
            pub const #ident: #ledger::FactPath<#name, #ty> = #ledger::FactPath::field(#index, #label, #kind, #flag_tokens, #link_tokens);
        });
        infos.push(quote!(#ledger::FieldInfo { name: #label, kind: #kind }));
    }
    if flag > 64 {
        return Err(syn::Error::new_spanned(name, "a record counts at most 64 yes-or-no fields"));
    }

    let idents: Vec<&syn::Ident> = fields.iter().map(|f| &f.ident).collect();
    let types: Vec<&Type> = fields.iter().map(|f| &f.ty).collect();
    let indexes: Vec<u16> = (0..fields.len() as u16).collect();
    let tuple_index: Vec<syn::Index> = (0..fields.len()).map(syn::Index::from).collect();
    let kinds: Vec<TokenStream> = fields.iter().map(|f| kind_tokens(f.kind)).collect();
    let all_names: Vec<String> = fields.iter().map(|f| f.ident.to_string()).collect();
    let saved: Vec<&Field> = fields.iter().filter(|f| matches!(f.kind, Kind::Document | Kind::Session)).collect();
    let saved_idents: Vec<&syn::Ident> = saved.iter().map(|f| &f.ident).collect();
    let saved_names: Vec<String> = saved.iter().map(|f| f.ident.to_string()).collect();
    let loads = fields.iter().map(|f| {
        let ident = &f.ident;
        let label = ident.to_string();
        if f.key {
            quote!(#ident: value.field(#label).and_then(#ledger::Field::from_value)?)
        } else if matches!(f.kind, Kind::Document | Kind::Session) {
            quote!(#ident: value.field(#label).and_then(#ledger::Field::from_value).unwrap_or_default())
        } else {
            quote!(#ident: ::std::default::Default::default())
        }
    });
    let documents: Vec<&Type> = fields.iter().filter(|f| f.kind == Kind::Document).map(|f| &f.ty).collect();
    let moving: Vec<TokenStream> = fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind == Kind::Live)
        .map(|(i, f)| {
            let (ident, index) = (&f.ident, i as u16);
            quote!(each(#index, &mut self.#ident);)
        })
        .collect();
    let mut derivations = Vec::new();
    let mut works = Vec::new();
    for (i, f) in named.named.iter().enumerate() {
        let Some(attr) = f.attrs.iter().find(|a| a.path().is_ident("derived")) else { continue };
        let (work, from) = attr.parse_args_with(|input: syn::parse::ParseStream| {
            let work: syn::ExprPath = input.parse()?;
            input.parse::<syn::Token![,]>()?;
            let word: syn::Ident = input.parse()?;
            if word != "from" {
                return Err(syn::Error::new_spanned(word, "write the fields it reads as `from = [a, b]`"));
            }
            input.parse::<syn::Token![=]>()?;
            let inside;
            syn::bracketed!(inside in input);
            let from = syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated(&inside)?;
            Ok((work, from.into_iter().collect::<Vec<_>>()))
        })?;
        let mut indexes = Vec::new();
        for name in &from {
            match fields.iter().position(|g| g.ident == *name) {
                Some(j) if j != i => indexes.push(j as u16),
                _ => return Err(syn::Error::new_spanned(name, "a derived value reads other fields of the same facts")),
            }
        }
        let (index, label) = (i as u16, fields[i].ident.to_string());
        derivations.push(quote!(#ledger::Derivation { field: #index, name: #label, from: &[#(#indexes),*] }));
        let names: Vec<syn::Ident> = (0..from.len()).map(|k| format_ident!("input{k}")).collect();
        works.push(quote! {
            #index => {
                let inputs = (#(::std::clone::Clone::clone(&self.#from),)*);
                ::std::option::Option::Some(::std::boxed::Box::new(move || {
                    let (#(#names,)*) = inputs;
                    ::std::boxed::Box::new(#work(#(&#names),*)) as ::std::boxed::Box<dyn ::std::any::Any + ::std::marker::Send>
                }))
            }
        });
    }

    let record_impl = record.map(|k| {
        let key = &k.ident;
        let flags = if flag_bits.is_empty() { quote!(0) } else { quote!(#(#flag_bits)|*) };
        let count = flag as usize;
        let link_count = links.len();
        let link_arms = links.iter().enumerate().map(|(n, ident)| quote!(#n => self.#ident.raw(),));
        quote! {
            impl #ledger::Record for #name {
                const FLAGS: usize = #count;
                const LINKS: usize = #link_count;
                fn link(&self, i: usize) -> u128 {
                    match i {
                        #(#link_arms)*
                        _ => 0,
                    }
                }
                fn key(&self) -> #ledger::Key<Self> {
                    self.#key
                }
                fn flags(&self) -> u64 {
                    #flags
                }
            }
        }
    });

    let mut seen: Vec<String> = Vec::new();
    let finds = fields.iter().filter_map(|f| {
        let item = list_item(&f.ty)?;
        let text = quote!(#item).to_string();
        if seen.contains(&text) {
            return None;
        }
        seen.push(text);
        let ident = &f.ident;
        Some(quote! {
            impl #ledger::Find<#item> for #name {
                fn find(&self, key: #ledger::Key<#item>) -> ::std::option::Option<&#item> {
                    self.#ident.find(key)
                }
            }
        })
    });

    let check = format_ident!("__graticule_document_fields_of_{}", name);
    Ok(quote! {
        impl #name {
            #(#consts)*
        }

        impl ::std::clone::Clone for #name {
            fn clone(&self) -> Self {
                Self { #(#idents: ::std::clone::Clone::clone(&self.#idents)),* }
            }
        }

        impl #ledger::Field for #name {
            type Snap = (#(<#types as #ledger::Field>::Snap,)*);

            fn snap(&self) -> Self::Snap {
                (#(#ledger::Field::snap(&self.#idents),)*)
            }

            fn diff(&self, before: &Self::Snap, at: #ledger::ErasedPath, out: &mut ::std::vec::Vec<#ledger::Op>) {
                #(#ledger::Field::diff(&self.#idents, &before.#tuple_index, at.push(#ledger::Step::Field(#indexes), #kinds), out);)*
            }

            fn same(&self, other: &Self) -> bool {
                true #(&& #ledger::Field::same(&self.#idents, &other.#idents))*
            }

            fn value(&self) -> #ledger::Value {
                #ledger::Value::Map(::std::vec![#((::std::string::String::from(#saved_names), #ledger::Field::value(&self.#saved_idents))),*])
            }

            fn from_value(value: &#ledger::Value) -> ::std::option::Option<Self> {
                if !::std::matches!(value, #ledger::Value::Map(_)) {
                    return ::std::option::Option::None;
                }
                ::std::option::Option::Some(Self { #(#loads),* })
            }

            fn attach(&mut self, at: #ledger::ErasedPath) {
                #(#ledger::Field::attach(&mut self.#idents, at.push(#ledger::Step::Field(#indexes), #kinds));)*
            }

            fn apply(&mut self, steps: &[#ledger::Step], op: &#ledger::Op) -> bool {
                match steps {
                    [] => match op {
                        #ledger::Op::Set { after, .. } => match <Self as #ledger::Field>::from_value(after) {
                            ::std::option::Option::Some(v) => {
                                *self = v;
                                true
                            }
                            ::std::option::Option::None => false,
                        },
                        _ => false,
                    },
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::apply(&mut self.#idents, rest, op),)*
                    _ => false,
                }
            }

            fn moving(&mut self, each: &mut dyn FnMut(u16, &mut dyn #ledger::Moving)) {
                let _ = &each;
                #(#moving)*
            }

            fn derivations() -> &'static [#ledger::Derivation] {
                &[#(#derivations),*]
            }

            fn derive_work(&self, field: u16) -> ::std::option::Option<#ledger::Work> {
                match field {
                    #(#works)*
                    _ => ::std::option::Option::None,
                }
            }

            fn prune(&mut self, deleted: &[(::std::any::TypeId, u128)]) {
                #(#ledger::Field::prune(&mut self.#idents, deleted);)*
            }

            fn edit(&mut self, steps: &[#ledger::Step], change: &#ledger::Change) -> #ledger::Outcome {
                match steps {
                    [] => match change {
                        #ledger::Change::Set(v) => match <Self as #ledger::Field>::from_value(v) {
                            ::std::option::Option::Some(new) => {
                                *self = new;
                                #ledger::Outcome::Changed
                            }
                            ::std::option::Option::None => #ledger::Outcome::Nothing,
                        },
                        _ => #ledger::Outcome::Nothing,
                    },
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::edit(&mut self.#idents, rest, change),)*
                    _ => #ledger::Outcome::Nothing,
                }
            }

            fn item_type(&self, steps: &[#ledger::Step]) -> ::std::option::Option<::std::any::TypeId> {
                match steps {
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::item_type(&self.#idents, rest),)*
                    _ => ::std::option::Option::None,
                }
            }

            fn item_keys(&self, steps: &[#ledger::Step]) -> ::std::option::Option<::std::vec::Vec<u128>> {
                match steps {
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::item_keys(&self.#idents, rest),)*
                    _ => ::std::option::Option::None,
                }
            }

            fn field_named(&self, steps: &[#ledger::Step], name: &str) -> ::std::option::Option<(u16, #ledger::Kind)> {
                match steps {
                    [] => match name {
                        #(#all_names => ::std::option::Option::Some((#indexes, #kinds)),)*
                        _ => ::std::option::Option::None,
                    },
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::field_named(&self.#idents, rest, name),)*
                    _ => ::std::option::Option::None,
                }
            }

            fn reach(&self, steps: &[#ledger::Step]) -> ::std::option::Option<&dyn ::std::any::Any> {
                match steps {
                    [] => ::std::option::Option::Some(self),
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::reach(&self.#idents, rest),)*
                    _ => ::std::option::Option::None,
                }
            }

            fn reach_mut(&mut self, steps: &[#ledger::Step]) -> ::std::option::Option<&mut dyn ::std::any::Any> {
                match steps {
                    [] => ::std::option::Option::Some(self),
                    #([#ledger::Step::Field(#indexes), rest @ ..] => #ledger::Field::reach_mut(&mut self.#idents, rest),)*
                    _ => ::std::option::Option::None,
                }
            }
        }

        impl #ledger::Facts for #name {
            const FIELDS: &'static [#ledger::FieldInfo] = &[#(#infos),*];
        }

        impl #ledger::Document for #name {}

        #record_impl

        #(#finds)*

        #[allow(non_snake_case, dead_code)]
        fn #check() {
            fn document<T: #ledger::Document>() {}
            #(document::<#documents>();)*
        }
    })
}

fn derive_enum(input: &DeriveInput, data: &syn::DataEnum) -> syn::Result<TokenStream> {
    let ledger = crate::ledger();
    let name = &input.ident;
    let mut clones = Vec::new();
    let mut sames = Vec::new();
    let mut values = Vec::new();
    let mut loads = Vec::new();
    for v in &data.variants {
        let vi = &v.ident;
        let label = vi.to_string();
        match &v.fields {
            Fields::Unit => {
                clones.push(quote!(Self::#vi => Self::#vi));
                sames.push(quote!((Self::#vi, Self::#vi) => true));
                values.push(quote!(Self::#vi => #ledger::Value::Variant(::std::string::String::from(#label), ::std::boxed::Box::new(#ledger::Value::Unit))));
                loads.push(quote!(#label => ::std::option::Option::Some(Self::#vi)));
            }
            Fields::Unnamed(f) => {
                let args: Vec<syn::Ident> = (0..f.unnamed.len()).map(|i| format_ident!("a{i}")).collect();
                let others: Vec<syn::Ident> = (0..f.unnamed.len()).map(|i| format_ident!("b{i}")).collect();
                let types: Vec<&Type> = f.unnamed.iter().map(|f| &f.ty).collect();
                let indexes: Vec<usize> = (0..f.unnamed.len()).collect();
                clones.push(quote!(Self::#vi(#(#args),*) => Self::#vi(#(::std::clone::Clone::clone(#args)),*)));
                sames.push(quote!((Self::#vi(#(#args),*), Self::#vi(#(#others),*)) => true #(&& #ledger::Field::same(#args, #others))*));
                values.push(quote!(Self::#vi(#(#args),*) => #ledger::Value::Variant(::std::string::String::from(#label), ::std::boxed::Box::new(#ledger::Value::List(::std::vec![#(#ledger::Field::value(#args)),*])))));
                loads.push(quote!(#label => match &**inner {
                    #ledger::Value::List(items) => ::std::option::Option::Some(Self::#vi(#(<#types as #ledger::Field>::from_value(items.get(#indexes)?)?),*)),
                    _ => ::std::option::Option::None,
                }));
            }
            Fields::Named(_) => return Err(syn::Error::new_spanned(vi, "variants of facts are unit or tuple variants")),
        }
    }
    Ok(quote! {
        impl ::std::clone::Clone for #name {
            fn clone(&self) -> Self {
                match self { #(#clones),* }
            }
        }

        impl #ledger::Field for #name {
            #ledger::register!(#name);

            #[allow(unreachable_patterns)]
            fn same(&self, other: &Self) -> bool {
                match (self, other) {
                    #(#sames,)*
                    _ => false,
                }
            }

            fn value(&self) -> #ledger::Value {
                match self { #(#values),* }
            }

            fn from_value(value: &#ledger::Value) -> ::std::option::Option<Self> {
                let #ledger::Value::Variant(name, inner) = value else { return ::std::option::Option::None };
                let _ = inner;
                match name.as_str() {
                    #(#loads,)*
                    _ => ::std::option::Option::None,
                }
            }
        }

        impl #ledger::Document for #name {}
    })
}
