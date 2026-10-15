use std::borrow::Cow;
use std::fmt;

/// Text for the user, made by `t!`: an id for translation, the English source and the text with its values filled in.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Message {
    id: &'static str,
    source: &'static str,
    text: Cow<'static, str>,
}

impl Message {
    pub const fn fixed(id: &'static str, source: &'static str) -> Self {
        Self { id, source, text: Cow::Borrowed(source) }
    }

    /// A message with values put in: its translation when the language in use has one, else `text`, the English formatted already.
    pub fn formatted<'n>(id: &'static str, source: &'static str, text: String, args: impl FnOnce() -> Vec<(&'n str, crate::language::Arg)>) -> Self {
        let text = crate::language::translated_with(id, args).unwrap_or(text);
        Self { id, source, text: Cow::Owned(text) }
    }

    pub fn plain(text: impl Into<Cow<'static, str>>) -> Self {
        Self { id: "", source: "", text: text.into() }
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn source(&self) -> &'static str {
        self.source
    }

    /// The text to show: the translation of a message without values when the language in use has one, else the text as written.
    pub fn as_str(&self) -> &str {
        match &self.text {
            Cow::Borrowed(_) => crate::language::fixed_text(self.id).unwrap_or(&self.text),
            Cow::Owned(text) => text,
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.text, f)
    }
}

impl From<&'static str> for Message {
    fn from(s: &'static str) -> Self {
        Message { id: "", source: s, text: Cow::Borrowed(s) }
    }
}

impl AsRef<str> for Message {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
