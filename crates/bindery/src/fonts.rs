use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sha256;

const PINNED: &str = include_str!("../../kernel/assets/fonts/fetched.txt");

struct Pinned<'p> {
    sum: &'p str,
    url: &'p str,
    name: &'p str,
}

fn pinned() -> Vec<Pinned<'static>> {
    PINNED
        .lines()
        .filter_map(|line| {
            let (sum, url) = line.trim().split_once(' ')?;
            let name = url.rsplit('/').next()?;
            Some(Pinned { sum, url: url.trim(), name })
        })
        .collect()
}

pub fn folder() -> Result<PathBuf, String> {
    dirs::data_dir().map(|d| d.join("Graticule").join("Fonts")).ok_or_else(|| "this system names no folder for app data".to_string())
}

pub fn fetch(from: Option<&Path>) -> Result<(), String> {
    let into = folder()?;
    std::fs::create_dir_all(&into).map_err(|e| format!("{}: {e}", into.display()))?;
    let mut failed = Vec::new();
    for font in pinned() {
        let path = into.join(font.name);
        if std::fs::read(&path).is_ok_and(|bytes| sha256::hex(&bytes) == font.sum) {
            println!("  {} is here already", font.name);
            continue;
        }
        let bytes = match from {
            Some(dir) => std::fs::read(dir.join(font.name)).map_err(|e| format!("{}: {e}", dir.join(font.name).display())),
            None => download(font.url),
        };
        let checked = bytes.and_then(|bytes| {
            let sum = sha256::hex(&bytes);
            if sum == font.sum { Ok(bytes) } else { Err(format!("its checksum is {sum}, not the pinned {}", font.sum)) }
        });
        let kept = checked.and_then(|bytes| {
            let part = path.with_extension("part");
            std::fs::write(&part, &bytes).and_then(|()| std::fs::rename(&part, &path)).map_err(|e| format!("{}: {e}", path.display())).map(|()| bytes.len())
        });
        match kept {
            Ok(size) => println!("  {} fetched, {:.1} MB", font.name, size as f64 / 1_048_576.0),
            Err(why) => {
                eprintln!("  {} not fetched: {why}", font.name);
                failed.push(font.name);
            }
        }
    }
    match failed.len() {
        0 => {
            println!("The fonts are in {}.", into.display());
            Ok(())
        }
        n => Err(format!("{n} of the fonts couldn't be fetched, and nothing was kept of them")),
    }
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl").args(["--fail", "--silent", "--show-error", "--location", url]).output().map_err(|e| format!("curl couldn't start: {e}"))?;
    if out.status.success() { Ok(out.stdout) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}
