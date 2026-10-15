//! Languages: an app's translations as Fluent data, the one in use chosen from the system's, and every label and message looked up in it by its id.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, LazyLock, RwLock};

use arc_swap::ArcSwapOption;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};

/// An app's translations: each a language's tag and its Fluent source, usually read in when the app is built: `&[("de", include_str!("../locales/de.ftl"))]`. English is written in the source and needs no file.
pub type Translations = &'static [(&'static str, &'static str)];

/// A value put into a message, which Fluent reads as a number when it is one, so it can choose the plural.
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Number(f64),
    Text(String),
}

/// Lets `t!` and labels pass any value into a message: `(&Wrap(&value)).arg()` gives a number for numbers and the displayed text for anything else.
pub struct Wrap<'a, T: ?Sized>(pub &'a T);

/// A value as a message takes it. It is implemented for `Wrap` of each kind of number and for a reference to `Wrap` of anything that displays, so a number finds its own first.
pub trait Argument {
    fn arg(&self) -> Arg;
}

macro_rules! numbers {
    ($($t:ty),*) => {
        $(impl Argument for Wrap<'_, $t> {
            fn arg(&self) -> Arg {
                Arg::Number(*self.0 as f64)
            }
        })*
    };
}

numbers!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize, f32, f64);

impl<T: Display + ?Sized> Argument for &Wrap<'_, T> {
    fn arg(&self) -> Arg {
        Arg::Text(self.0.to_string())
    }
}

struct Language {
    bundle: FluentBundle<FluentResource>,
    fixed: RwLock<HashMap<String, Option<&'static str>>>,
}

static CURRENT: LazyLock<ArcSwapOption<Language>> = LazyLock::new(ArcSwapOption::empty);

/// Uses the translation for `tag`, a language tag like `de` or `pt-BR`, or the closest one there is: the same language in another place, else English, written in the source. Says which it chose, if any, and what in its file couldn't be read.
pub fn choose(translations: Translations, tag: &str) -> (Option<&'static str>, Vec<String>) {
    let want = tag.replace('_', "-").to_lowercase();
    let base = want.split('-').next().unwrap_or_default().to_string();
    let found = translations.iter().find(|(t, _)| t.to_lowercase() == want).or_else(|| translations.iter().find(|(t, _)| t.to_lowercase().split('-').next() == Some(base.as_str())));
    let Some(&(tag, source)) = found else {
        CURRENT.store(None);
        return (None, Vec::new());
    };
    let mut troubles = Vec::new();
    let resource = FluentResource::try_new(source.to_string()).unwrap_or_else(|(resource, errors)| {
        troubles.extend(errors.iter().map(|e| format!("{tag}: {e}")));
        resource
    });
    let id = tag.parse().unwrap_or_default();
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    bundle.set_use_isolating(false);
    if let Err(errors) = bundle.add_resource(resource) {
        troubles.extend(errors.iter().map(|e| format!("{tag}: {e}")));
    }
    CURRENT.store(Some(Arc::new(Language { bundle, fixed: RwLock::default() })));
    (Some(tag), troubles)
}

/// Uses the system's language, if the app has a translation for it.
pub fn choose_system(translations: Translations) -> (Option<&'static str>, Vec<String>) {
    match sys_locale::get_locale() {
        Some(tag) => choose(translations, &tag),
        None => (None, Vec::new()),
    }
}

/// Goes back to English, written in the source.
pub fn english() {
    CURRENT.store(None);
}

/// The translation of a message that needs no values, if the language in use has one. Each is formatted the first time it is asked for and kept for as long as the language is in use.
pub fn fixed_text(id: &str) -> Option<&'static str> {
    if id.is_empty() {
        return None;
    }
    let current = CURRENT.load();
    let language = current.as_ref()?;
    if let Some(known) = language.fixed.read().ok()?.get(id) {
        return *known;
    }
    let text = translated(id, &[]).map(|t| &*Box::leak(t.into_boxed_str()));
    language.fixed.write().ok()?.insert(id.to_string(), text);
    text
}

/// The translation of a message with `args` put in, if the language in use has one.
pub fn translated(id: &str, args: &[(&str, Arg)]) -> Option<String> {
    translated_with(id, || args.to_vec())
}

/// The translation of a message with the values `args` gives put in, if the language in use has one; `args` is asked only then.
pub fn translated_with<'n>(id: &str, args: impl FnOnce() -> Vec<(&'n str, Arg)>) -> Option<String> {
    if id.is_empty() {
        return None;
    }
    let current = CURRENT.load();
    let language = current.as_ref()?;
    let pattern = language.bundle.get_message(id)?.value()?;
    let args = args();
    let mut fluent = FluentArgs::new();
    for (name, arg) in args {
        match arg {
            Arg::Number(n) => fluent.set(name.to_string(), FluentValue::from(n)),
            Arg::Text(t) => fluent.set(name.to_string(), FluentValue::from(t)),
        }
    }
    let mut errors = Vec::new();
    let text = language.bundle.format_pattern(pattern, Some(&fluent), &mut errors);
    errors.is_empty().then(|| text.into_owned())
}
