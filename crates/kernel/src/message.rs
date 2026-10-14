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

    pub fn formatted(id: &'static str, source: &'static str, text: String) -> Self {
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

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
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
        &self.text
    }
}
