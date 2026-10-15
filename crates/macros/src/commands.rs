use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Data, DeriveInput, Fields, LitStr, Token, Type};

#[derive(Default)]
struct Label {
    plain: Option<LitStr>,
    by: Option<(syn::Ident, LitStr, LitStr)>,
    short: Option<LitStr>,
    many: Option<LitStr>,
}

impl Parse for Label {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut label = Label::default();
        let mut when: Option<(syn::Ident, LitStr)> = None;
        let mut otherwise: Option<LitStr> = None;
        while !input.is_empty() {
            if input.peek(LitStr) {
                label.plain = Some(input.parse()?);
            } else if input.peek(Token![else]) {
                input.parse::<Token![else]>()?;
                input.parse::<Token![:]>()?;
                otherwise = Some(input.parse()?);
            } else {
                let name: syn::Ident = input.parse()?;
                input.parse::<Token![:]>()?;
                let text: LitStr = input.parse()?;
                match name.to_string().as_str() {
                    "short" => label.short = Some(text),
                    "many" => label.many = Some(text),
                    _ => when = Some((name, text)),
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        match (when, otherwise) {
            (Some((field, yes)), Some(no)) => label.by = Some((field, yes, no)),
            (None, None) => {}
            (Some((field, _)), None) => return Err(syn::Error::new_spanned(field, "a label chosen by a field also says what it is `else`")),
            (None, Some(no)) => return Err(syn::Error::new_spanned(no, "`else` needs a label chosen by a field first, like `done: \"…\"`")),
        }
        if label.plain.is_none() && label.by.is_none() {
            return Err(input.error("a label needs its text"));
        }
        Ok(label)
    }
}

enum Hole {
    Text(String),
    Name(String),
}

fn holes(text: &str) -> Vec<Hole> {
    let mut out = Vec::new();
    let mut lit = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push('}');
            }
            '{' => {
                if !lit.is_empty() {
                    out.push(Hole::Text(std::mem::take(&mut lit)));
                }
                let name: String = chars.by_ref().take_while(|c| *c != '}').collect();
                out.push(Hole::Name(name.trim().to_string()));
            }
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        out.push(Hole::Text(lit));
    }
    out
}

fn key_item(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Key" {
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

fn is_edit(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Edit"))
}

struct Arg {
    ident: syn::Ident,
    name: Option<syn::Ident>,
    ty: Type,
    subject: bool,
}

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let kernel = crate::kernel();
    let ledger = crate::ledger();
    let name = &input.ident;
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(name, "commands are an enum, one variant per command"));
    };
    let mut names = Vec::new();
    let mut labels = Vec::new();
    let mut internals = Vec::new();
    let mut subjects = Vec::new();
    let mut with_subjects = Vec::new();
    let mut manys = Vec::new();
    let mut entries = Vec::new();
    let mut makes = Vec::new();
    let mut values = Vec::new();
    let mut clones = Vec::new();
    let mut loads = Vec::new();
    let mut bounds: Vec<Type> = Vec::new();
    let mut edit_from = None;
    let mut edit_as = None;
    for (index, v) in data.variants.iter().enumerate() {
        let vi = &v.ident;
        let command_name = messages::kebab(&vi.to_string());
        let internal = v.attrs.iter().any(|a| a.path().is_ident("internal"));
        let doc = v
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("doc"))
            .filter_map(|a| match &a.meta {
                syn::Meta::NameValue(syn::MetaNameValue { value: syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }), .. }) => Some(s.value().trim().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
        let label: Option<Label> = v.attrs.iter().find(|a| a.path().is_ident("label")).map(|a| a.parse_args()).transpose()?;
        if label.is_none() && !internal {
            return Err(syn::Error::new_spanned(vi, "every command the user can reach has a #[label(\"…\")]; mark the others #[internal]"));
        }
        let (args, named) = match &v.fields {
            Fields::Unit => (Vec::new(), false),
            Fields::Unnamed(f) => (
                f.unnamed.iter().enumerate().map(|(i, f)| Arg { ident: format_ident!("a{i}"), name: None, ty: f.ty.clone(), subject: f.attrs.iter().any(|a| a.path().is_ident("subject")) }).collect(),
                false,
            ),
            Fields::Named(f) => (
                f.named
                    .iter()
                    .map(|f| Arg { ident: f.ident.clone().expect("named"), name: f.ident.clone(), ty: f.ty.clone(), subject: f.attrs.iter().any(|a| a.path().is_ident("subject")) })
                    .collect(),
                true,
            ),
        };
        let idents: Vec<&syn::Ident> = args.iter().map(|a| &a.ident).collect();
        let pattern = match (&v.fields, named) {
            (Fields::Unit, _) => quote!(Self::#vi),
            (_, false) => quote!(Self::#vi(#(#idents),*)),
            (_, true) => quote!(Self::#vi { #(#idents),* }),
        };
        let ignore = match &v.fields {
            Fields::Unit => quote!(Self::#vi),
            Fields::Unnamed(_) => quote!(Self::#vi(..)),
            Fields::Named(_) => quote!(Self::#vi { .. }),
        };
        let subject = args.iter().find(|a| a.subject);
        if args.iter().filter(|a| a.subject).count() > 1 {
            return Err(syn::Error::new_spanned(vi, "a command has at most one #[subject]"));
        }
        if args.len() == 1 && is_edit(&args[0].ty) && !named {
            edit_from = Some(quote!(::std::option::Option::Some(Self::#vi(edit))));
            edit_as = Some(quote!(Self::#vi(edit) => ::std::option::Option::Some(edit),));
        }
        names.push(quote!(#ignore => #command_name));
        internals.push(quote!(#ignore => #internal));
        let subject_item = subject.and_then(|s| key_item(&s.ty));
        if let Some(item) = subject_item
            && !bounds.iter().any(|b| quote!(#b).to_string() == quote!(#item).to_string())
        {
            bounds.push(item.clone());
        }

        match subject {
            Some(s) => {
                let si = &s.ident;
                let raw = if subject_item.is_some() { quote!(#si.raw()) } else { quote!(#kernel::Subject::raw(#si)) };
                subjects.push(quote!(#pattern => ::std::option::Option::Some(#raw)));
                let rebuilt: Vec<TokenStream> = args
                    .iter()
                    .map(|a| {
                        let ai = &a.ident;
                        let value = if a.subject {
                            if subject_item.is_some() { quote!(#ledger::Key::from_raw(subject)) } else { quote!(#kernel::Subject::from_raw(subject)) }
                        } else {
                            quote!(::std::clone::Clone::clone(#ai))
                        };
                        match &a.name {
                            Some(n) => quote!(#n: #value),
                            None => value,
                        }
                    })
                    .collect();
                let built = if named { quote!(Self::#vi { #(#rebuilt),* }) } else { quote!(Self::#vi(#(#rebuilt),*)) };
                with_subjects.push(quote!(#pattern => #built));
            }
            None => {
                subjects.push(quote!(#ignore => ::std::option::Option::None));
                with_subjects.push(quote!(#ignore => ::std::clone::Clone::clone(self)));
            }
        }

        let has_many = label.as_ref().is_some_and(|l| l.many.is_some());
        manys.push(quote!(#ignore => #has_many));
        let takes_subject = subject.is_some();
        let mut given = 0usize;
        let made: Vec<TokenStream> = args
            .iter()
            .map(|a| {
                let ty = &a.ty;
                let value = if a.subject {
                    if subject_item.is_some() { quote!(#ledger::Key::from_raw(subject?)) } else { quote!(#kernel::Subject::from_raw(subject?)) }
                } else {
                    given += 1;
                    let i = given - 1;
                    quote!(<#ty as #ledger::Field>::from_value(args.get(#i)?)?)
                };
                match &a.name {
                    Some(n) => quote!(#n: #value),
                    None => value,
                }
            })
            .collect();
        if !internal {
            makes.push(match &v.fields {
                Fields::Unit => quote!(#index => ::std::option::Option::Some(Self::#vi)),
                Fields::Unnamed(_) => quote!(#index => ::std::option::Option::Some(Self::#vi(#(#made),*))),
                Fields::Named(_) => quote!(#index => ::std::option::Option::Some(Self::#vi { #(#made),* })),
            });
        }
        let asked: Vec<TokenStream> = if internal {
            Vec::new()
        } else {
            args.iter()
                .filter(|a| !a.subject)
                .map(|a| {
                    let ty = &a.ty;
                    let name = a.name.as_ref().map(|n| n.to_string().replace('_', " ")).unwrap_or_default();
                    quote!(#kernel::Arg { name: #name, kind: <#ty as #kernel::Argument>::KIND, read: #kernel::read_arg::<#ty> })
                })
                .collect()
        };
        let subject_type = match subject_item {
            Some(item) => quote!(::std::option::Option::Some(::std::any::TypeId::of::<#item> as fn() -> ::std::any::TypeId)),
            None => quote!(::std::option::Option::None),
        };
        entries.push(quote!(#kernel::Entry { name: #command_name, internal: #internal, subject: #takes_subject, subject_type: #subject_type, many: #has_many, args: &[#(#asked),*], doc: #doc }));

        let types: Vec<&Type> = args.iter().map(|a| &a.ty).collect();
        let cloned: Vec<TokenStream> = args
            .iter()
            .map(|a| {
                let ai = &a.ident;
                match &a.name {
                    Some(n) => quote!(#n: ::std::clone::Clone::clone(#ai)),
                    None => quote!(::std::clone::Clone::clone(#ai)),
                }
            })
            .collect();
        clones.push(match &v.fields {
            Fields::Unit => quote!(Self::#vi => Self::#vi),
            Fields::Unnamed(_) => quote!(#pattern => Self::#vi(#(#cloned),*)),
            Fields::Named(_) => quote!(#pattern => Self::#vi { #(#cloned),* }),
        });
        let positions: Vec<usize> = (0..args.len()).collect();
        values.push(
            quote!(#pattern => #ledger::Value::Variant(::std::string::String::from(#command_name), ::std::boxed::Box::new(#ledger::Value::List(::std::vec![#(#ledger::Field::value(#idents)),*])))),
        );
        let loaded: Vec<TokenStream> = args
            .iter()
            .zip(&types)
            .zip(&positions)
            .map(|((a, ty), i)| {
                let value = quote!(<#ty as #ledger::Field>::from_value(items.get(#i)?)?);
                match &a.name {
                    Some(n) => quote!(#n: #value),
                    None => value,
                }
            })
            .collect();
        let load = match &v.fields {
            Fields::Unit => quote!(Self::#vi),
            Fields::Unnamed(_) => quote!(Self::#vi(#(#loaded),*)),
            Fields::Named(_) => quote!(Self::#vi { #(#loaded),* }),
        };
        loads.push(quote!(#command_name => ::std::option::Option::Some(#load)));

        let label_body = match &label {
            None => quote!(out.push_str(#command_name)),
            Some(label) => {
                let write = |text: &LitStr, many: bool, id: String| -> syn::Result<TokenStream> {
                    let (mut parts, mut values) = (Vec::new(), Vec::new());
                    for hole in holes(&text.value()) {
                        let value = match &hole {
                            Hole::Text(t) => {
                                parts.push(quote!(out.push_str(#t);));
                                continue;
                            }
                            Hole::Name(n) if n == "count" && many => quote!(count),
                            Hole::Name(n) => {
                                let field = format_ident!("{}", n);
                                if let Some(arg) = args.iter().find(|a| a.name.as_ref().is_some_and(|x| x == n)) {
                                    let ai = &arg.ident;
                                    quote!(#ai)
                                } else if subject_item.is_some() {
                                    quote!(found.#field)
                                } else if let Some(s) = subject {
                                    let si = &s.ident;
                                    quote!(#si)
                                } else {
                                    return Err(syn::Error::new_spanned(text, format!("`{{{n}}}` isn't an argument of this command, and it has no subject to read it from")));
                                }
                            }
                        };
                        let Hole::Name(n) = hole else { continue };
                        parts.push(quote!(let _ = ::std::write!(out, "{}", #value);));
                        values.push(quote!((#n, (&#kernel::language::Wrap(&#value)).arg())));
                    }
                    Ok(quote! {
                        match #kernel::language::translated_with(#id, || {
                            use #kernel::language::Argument as _;
                            ::std::vec![#(#values),*]
                        }) {
                            ::std::option::Option::Some(translated) => out.push_str(&translated),
                            ::std::option::Option::None => { #(#parts)* }
                        }
                    })
                };
                let needs_found = |text: &LitStr| {
                    subject_item.is_some() && holes(&text.value()).iter().any(|h| matches!(h, Hole::Name(n) if n != "count" && !args.iter().any(|a| a.name.as_ref().is_some_and(|x| x == n))))
                };
                let fallback = match (&label.short, &label.plain, &label.by) {
                    (Some(s), _, _) => quote!(out.push_str(#s);),
                    (None, Some(p), _) => {
                        let stripped: String = holes(&p.value())
                            .iter()
                            .map(|h| match h {
                                Hole::Text(t) => t.as_str(),
                                Hole::Name(_) => "…",
                            })
                            .collect();
                        quote!(out.push_str(#stripped);)
                    }
                    (None, None, Some((_, yes, _))) => {
                        let stripped: String = holes(&yes.value())
                            .iter()
                            .map(|h| match h {
                                Hole::Text(t) => t.as_str(),
                                Hole::Name(_) => "…",
                            })
                            .collect();
                        quote!(out.push_str(#stripped);)
                    }
                    (None, None, None) => quote!(out.push_str(#command_name);),
                };
                let long = match (&label.plain, &label.by) {
                    (_, Some((field, yes, no))) => {
                        let yes_w = write(yes, false, command_name.clone())?;
                        let no_w = write(no, false, format!("{command_name}-else"))?;
                        if subject_item.is_none() {
                            return Err(syn::Error::new_spanned(field, "a label chosen by a field reads it from the command's #[subject] key"));
                        }
                        let s = &subject.expect("subject").ident;
                        quote! {
                            match facts.find(*#s) {
                                ::std::option::Option::Some(found) => if found.#field { #yes_w } else { #no_w },
                                ::std::option::Option::None => { #fallback }
                            }
                        }
                    }
                    (Some(plain), None) => {
                        let w = write(plain, false, command_name.clone())?;
                        if needs_found(plain) {
                            let s = &subject.expect("subject").ident;
                            quote! {
                                match facts.find(*#s) {
                                    ::std::option::Option::Some(found) => { #w }
                                    ::std::option::Option::None => { #fallback }
                                }
                            }
                        } else {
                            w
                        }
                    }
                    (None, None) => unreachable!(),
                };
                let short = match &label.short {
                    Some(s) => write(s, false, format!("{command_name}-short"))?,
                    None => long.clone(),
                };
                let many = match &label.many {
                    Some(m) => write(m, true, format!("{command_name}-many"))?,
                    None => long.clone(),
                };
                quote! {
                    match form {
                        #kernel::Form::Long => { #long }
                        #kernel::Form::Short => { #short }
                        #kernel::Form::Many(count) => { let _ = count; #many }
                    }
                }
            }
        };
        labels.push(quote!(#[allow(unused_variables)] #pattern => { #label_body }));
    }
    let edit_from = edit_from.unwrap_or_else(|| {
        quote!({
            let _ = edit;
            ::std::option::Option::None
        })
    });
    let edit_as = edit_as.unwrap_or_default();
    let keymap = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) if std::path::Path::new(&dir).join("keymap.txt").exists() => quote! {
            fn keymap() -> &'static str {
                ::std::include_str!(::std::concat!(::std::env!("CARGO_MANIFEST_DIR"), "/keymap.txt"))
            }
        },
        _ => quote!(),
    };
    let bounds = bounds.iter().map(|b| quote!(F: #ledger::Find<#b>));
    Ok(quote! {
        impl ::std::clone::Clone for #name {
            fn clone(&self) -> Self {
                match self { #(#clones),* }
            }
        }

        impl #ledger::Field for #name {
            #ledger::register!(#name);

            fn same(&self, other: &Self) -> bool {
                #ledger::Field::value(self) == #ledger::Field::value(other)
            }

            fn value(&self) -> #ledger::Value {
                match self { #(#values),* }
            }

            fn from_value(value: &#ledger::Value) -> ::std::option::Option<Self> {
                let #ledger::Value::Variant(name, inner) = value else { return ::std::option::Option::None };
                let #ledger::Value::List(items) = &**inner else { return ::std::option::Option::None };
                let _ = items;
                match name.as_str() {
                    #(#loads,)*
                    _ => ::std::option::Option::None,
                }
            }
        }

        impl<F> #kernel::Commands<F> for #name where #(#bounds),* {
            fn name(&self) -> &'static str {
                match self { #(#names),* }
            }

            fn label(&self, facts: &F, form: #kernel::Form, out: &mut ::std::string::String) {
                use ::std::fmt::Write as _;
                let _ = facts;
                match self { #(#labels),* }
            }

            fn internal(&self) -> bool {
                match self { #(#internals),* }
            }

            fn subject(&self) -> ::std::option::Option<u128> {
                match self { #(#subjects),* }
            }

            fn with_subject(&self, subject: u128) -> Self {
                let _ = subject;
                match self { #(#with_subjects),* }
            }

            fn many(&self) -> bool {
                match self { #(#manys),* }
            }

            fn edit(edit: #ledger::Edit) -> ::std::option::Option<Self> {
                #edit_from
            }

            fn as_edit(&self) -> ::std::option::Option<&#ledger::Edit> {
                #[allow(unreachable_patterns)]
                match self {
                    #edit_as
                    _ => ::std::option::Option::None,
                }
            }

            #keymap

            fn entries() -> &'static [#kernel::Entry] {
                &[#(#entries),*]
            }

            fn make(entry: usize, subject: ::std::option::Option<u128>, args: &[#ledger::Value]) -> ::std::option::Option<Self> {
                let _ = (subject, args);
                match entry {
                    #(#makes,)*
                    _ => ::std::option::Option::None,
                }
            }
        }
    })
}
