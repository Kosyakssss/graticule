use std::path::Path;

use crate::changes::{Sheet, Verdict};

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn link(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '?' => "%3F".to_string(),
            '\\' => "/".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// The reference images pinned to a state: files in `folder` named after it ("Empty.png", "Empty, from the mockup.jpg").
fn references(folder: Option<&Path>, state: &str) -> Vec<std::path::PathBuf> {
    let Some(entries) = folder.and_then(|f| std::fs::read_dir(f).ok()) else { return Vec::new() };
    let mut found: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let picture = p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp"));
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            picture && (stem == state || stem.starts_with(&format!("{state}, ")))
        })
        .collect();
    found.sort();
    found
}

fn words(sheets: &[Sheet], name: &str) -> String {
    match sheets.iter().find(|s| s.name == name) {
        None => String::new(),
        Some(s) => match s.verdict {
            Verdict::Same | Verdict::Approved => "<p class=same>Renders as approved.</p>".to_string(),
            Verdict::New => "<p class=changed>Never approved.</p>".to_string(),
            Verdict::Changed => format!("<ul class=changed>{}</ul>", s.said.lines().map(|l| format!("<li>{}</li>", escape(l))).collect::<String>()),
        },
    }
}

/// Writes `index.html` in `folder`: every named state rendered there, light and dark, at its normal and smallest size, with what changed since it was approved and any reference images pinned beside it, for review without a window.
pub fn write(folder: &Path, app: &str, states: &[&str], sheets: &[Sheet], pinned: Option<&Path>) -> Result<(), String> {
    let mut body = String::new();
    for state in states {
        body.push_str(&format!("<section><h2>{}</h2>", escape(state)));
        for size in ["default", "smallest"] {
            let name = format!("{state}, {size}");
            body.push_str(&format!("<h3>{}</h3>{}<div class=row>", if size == "default" { "Normal size" } else { "Smallest window" }, words(sheets, &name)));
            for look in ["light", "dark"] {
                let file = format!("{name}, {look}.png");
                if folder.join(&file).is_file() {
                    body.push_str(&format!("<figure><img src=\"{}\" alt=\"{}\"><figcaption>{look}</figcaption></figure>", link(&file), escape(&file)));
                }
            }
            body.push_str("</div>");
        }
        let pins = references(pinned, state);
        if !pins.is_empty() {
            body.push_str("<h3>Reference</h3><div class=row>");
            for pin in pins {
                let shown = pin.display().to_string();
                body.push_str(&format!("<figure><img src=\"file:///{}\" alt=\"{}\"><figcaption>{}</figcaption></figure>", link(shown.trim_start_matches('/')), escape(&shown), escape(&shown)));
            }
            body.push_str("</div>");
        }
        body.push_str("</section>");
    }
    let html = format!(
        "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><title>{app}: Specimen</title><style>{STYLE}</style><h1>{app}</h1><p>Every named state, light and dark, at its normal size and in the smallest window it allows. Under each, what renders differently from the proof last approved.</p>{body}</html>",
        app = escape(app)
    );
    std::fs::write(folder.join("index.html"), html).map_err(|e| format!("index.html couldn't be saved: {e}"))
}

const STYLE: &str = ":root{color-scheme:light dark;--ink:#222;--muted:#666;--bg:#f8f7f5;--rule:#ccc;--changed:#a33}@media (prefers-color-scheme:dark){:root{--ink:#ddd;--muted:#999;--bg:#1b1b1d;--rule:#444;--changed:#f99}}body{font:15px/1.5 system-ui,sans-serif;color:var(--ink);background:var(--bg);margin:2rem;max-width:none}section{border-top:1px solid var(--rule);padding-top:1rem;margin-top:2rem}h3{font-size:1rem;color:var(--muted);margin:1rem 0 .25rem}.row{display:flex;flex-wrap:wrap;gap:1rem;align-items:flex-start}figure{margin:0}img{max-width:min(100%,960px);border:1px solid var(--rule);display:block}figcaption{color:var(--muted);font-size:.85rem}.same{color:var(--muted)}.changed{color:var(--changed)}";
