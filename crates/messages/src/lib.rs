//! How a label or a message gets its id, shared by the macros that write the ids and the tool that finds them; and the tool's side: every label and message in an app's source, written out as Fluent.

use std::path::{Path, PathBuf};

use syn::visit::Visit;

/// The id of a message written as `text` at a call site: its first words, then a short hash of all of it, so the same words give the same id and a changed text a new one.
pub fn id_of(text: &str) -> String {
    let mut id = String::new();
    for word in text.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).take(6) {
        if !id.is_empty() {
            id.push('-');
        }
        id.push_str(&word.to_ascii_lowercase());
    }
    if !id.starts_with(|c: char| c.is_ascii_alphabetic()) {
        id.insert_str(0, if id.is_empty() { "text" } else { "text-" });
    }
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{id}-{:04x}", hash & 0xffff)
}

/// A command's name from its variant: `MarkDone` is `mark-done`.
pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Whether a text puts values in, like `{count}`; `{{` is a brace written out.
pub fn has_args(text: &str) -> bool {
    !names(text).is_empty()
}

/// The names of the values a text puts in, each once, in the order they first come: `{count}` and `{size:.1}` give `count` and `size`.
pub fn names(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '{' => {
                let inside: String = chars.by_ref().take_while(|c| *c != '}').collect();
                let name = inside.split(':').next().unwrap_or_default().trim().to_string();
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
            _ => {}
        }
    }
    out
}

/// A text written the way Fluent writes it: `{count} posts` is `{ $count } posts`.
pub fn fluent(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push_str("{\"{\"}");
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push_str("{\"}\"}");
            }
            '{' => {
                let inside: String = chars.by_ref().take_while(|c| *c != '}').collect();
                out.push_str(&format!("{{ ${} }}", inside.split(':').next().unwrap_or_default().trim()));
            }
            c => out.push(c),
        }
    }
    out
}

/// A label or message found in the source: its id, its English text, and where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    pub text: String,
    pub file: PathBuf,
    pub line: usize,
}

/// Every `t!` message and every command's label in the Rust files under `folder`, in the order they are written.
pub fn find(folder: &Path) -> Result<Vec<Found>, String> {
    let mut files = Vec::new();
    walk(folder, &mut files)?;
    files.sort();
    let mut found = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let parsed = syn::parse_file(&source).map_err(|e| format!("{}: {e}", file.display()))?;
        let mut finder = Finder { file: file.clone(), found: Vec::new() };
        finder.visit_file(&parsed);
        found.extend(finder.found);
    }
    Ok(found)
}

fn walk(folder: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(folder).map_err(|e| format!("{}: {e}", folder.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if path.is_dir() && name != "target" && !name.starts_with('.') {
            walk(&path, files)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

struct Finder {
    file: PathBuf,
    found: Vec<Found>,
}

impl Finder {
    fn add(&mut self, id: String, text: String, at: proc_macro2::Span) {
        self.found.push(Found { id, text, file: self.file.clone(), line: at.start().line });
    }
}

impl<'a> Visit<'a> for Finder {
    fn visit_macro(&mut self, m: &'a syn::Macro) {
        if m.path.segments.last().is_some_and(|s| s.ident == "t")
            && let Ok(lit) = m.parse_body::<syn::LitStr>()
        {
            self.add(id_of(&lit.value()), lit.value(), lit.span());
        }
        syn::visit::visit_macro(self, m);
    }

    fn visit_item_enum(&mut self, e: &'a syn::ItemEnum) {
        let commands = e.attrs.iter().any(|a| {
            a.path().is_ident("derive")
                && a.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                    .is_ok_and(|paths| paths.iter().any(|p| p.segments.last().is_some_and(|s| s.ident == "Commands")))
        });
        if commands {
            for v in &e.variants {
                let name = kebab(&v.ident.to_string());
                for attr in v.attrs.iter().filter(|a| a.path().is_ident("label")) {
                    if let Ok(parts) = attr.parse_args_with(syn::punctuated::Punctuated::<Part, syn::Token![,]>::parse_terminated) {
                        for part in parts {
                            let id = match part.name.as_deref() {
                                None => name.clone(),
                                Some("short") => format!("{name}-short"),
                                Some("many") => format!("{name}-many"),
                                Some("else") => format!("{name}-else"),
                                Some(_) => name.clone(),
                            };
                            self.add(id, part.text.value(), part.text.span());
                        }
                    }
                }
            }
        }
        syn::visit::visit_item_enum(self, e);
    }
}

/// One text of a `#[label(…)]`: plain, or named like `short: "…"`, `done: "…"` or `else: "…"`.
struct Part {
    name: Option<String>,
    text: syn::LitStr,
}

impl syn::parse::Parse for Part {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.peek(syn::LitStr) {
            return Ok(Part { name: None, text: input.parse()? });
        }
        let name = if input.peek(syn::Token![else]) {
            input.parse::<syn::Token![else]>()?;
            "else".to_string()
        } else {
            input.parse::<syn::Ident>()?.to_string()
        };
        input.parse::<syn::Token![:]>()?;
        Ok(Part { name: Some(name), text: input.parse()? })
    }
}

/// The ids a Fluent file gives a translation, in order.
pub fn ids_in(ftl: &str) -> Vec<String> {
    ftl.lines().filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic())).filter_map(|l| l.split_once('=').map(|(id, _)| id.trim().to_string())).collect()
}

/// The English template for translators: every text found, once, under a comment that says where it is written.
pub fn template(found: &[Found], root: &Path) -> String {
    let mut out = String::from("# Every label and message of this app, in English as written in its source. Copy this file to a language's tag, like de.ftl, and translate each text.\n");
    let mut seen = Vec::new();
    for f in found {
        if seen.contains(&f.id) {
            continue;
        }
        seen.push(f.id.clone());
        let place = f.file.strip_prefix(root).unwrap_or(&f.file).to_string_lossy().replace('\\', "/");
        out.push_str(&format!("\n# {place}:{}\n{} = {}\n", f.line, f.id, fluent(&f.text)));
    }
    out
}
