use std::path::{Path, PathBuf};

use kernel::App;
use ledger::Value;

use crate::{Finding, Law};

/// Where an app's proofs are kept: `proofs/<app>` in the workspace it runs from, or in the app's own folder outside one.
pub fn proofs() -> PathBuf {
    let app = std::env::current_exe().ok().and_then(|exe| exe.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_else(|| "app".to_string());
    let workspace = std::env::current_dir().ok().and_then(|dir| dir.ancestors().find(|d| d.join("Cargo.lock").is_file()).map(Path::to_path_buf));
    match workspace {
        Some(root) => root.join("proofs").join(app),
        None => kernel::runner::folder().join("Proofs"),
    }
}

/// Keeps every named state's facts as this version saves them, in `facts/<shape>` among the app's proofs, so later versions can be checked against them. Facts kept for older shapes are never touched. Returns how many were written.
pub fn keep<A: App>() -> std::io::Result<usize> {
    let folder = proofs().join("facts").join(A::migrations().len().to_string());
    std::fs::create_dir_all(&folder)?;
    let states = A::states().list;
    for (name, facts) in &states {
        std::fs::write(folder.join(format!("{name}.value")), facts.value().to_bytes())?;
    }
    Ok(states.len())
}

pub(crate) fn check<A: App>(folder: &Path) -> (Vec<Finding>, usize) {
    let mut findings = Vec::new();
    let mut checked = 0;
    let migrations = A::migrations();
    let Ok(shapes) = std::fs::read_dir(folder) else { return (findings, checked) };
    for shape in shapes.flatten() {
        let Some(n) = shape.file_name().to_str().and_then(|s| s.parse::<usize>().ok()) else { continue };
        let Ok(files) = std::fs::read_dir(shape.path()) else { continue };
        for file in files.flatten() {
            let state = file.path().file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            checked += 1;
            let said = match (std::fs::read(file.path()).ok().and_then(|b| Value::from_bytes(&b).ok()), migrations.get(n..)) {
                (_, None) => format!("facts were kept for shape {n}, but the app has only {} migrations", migrations.len()),
                (None, _) => "the kept facts can't be read".to_string(),
                (Some(value), Some(later)) if A::from_value(&ledger::migrate(value.clone(), later)).is_none() => match later.len() {
                    0 => "the facts as this version saved them no longer load; if their shape changed on purpose, add a migration".to_string(),
                    m => format!("the facts as saved {m} shapes ago don't load after the migrations since"),
                },
                _ => continue,
            };
            findings.push(Finding { law: Law::SavedFactsLoad, said, from: format!("“{state}” as saved in shape {n}"), steps: Vec::new() });
        }
    }
    (findings, checked)
}
