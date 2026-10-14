//! BD-01, Bindery: starts a Graticule app with `bindery new`, and with `bindery dev` rebuilds it on every saved change and swaps the running app for the new build in the same window frame and state.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

const USAGE: &str = "BD-01 Bindery: starts Graticule apps and runs the development loop

  bindery new FOLDER
      makes an app there: facts, commands, one screen, one named state,
      Specimen wired in (run it with --specimen)

  bindery dev [FOLDER] [-p PACKAGE] [--bin NAME] [-- APP ARGUMENTS]
      builds the app, runs it, and on every saved change builds it again;
      a build that succeeds replaces the running app in the same window
      frame and state, and one that fails never reaches it
";

const RELOAD: &str = "GRATICULE_RELOAD";
const LOOK_EVERY: Duration = Duration::from_millis(250);
const GOODBYE: Duration = Duration::from_secs(5);

fn main() {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let (args, app_args) = match all.iter().position(|a| a == "--") {
        Some(i) => (all[..i].to_vec(), all[i + 1..].to_vec()),
        None => (all, Vec::new()),
    };
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let mut positional = Vec::new();
    let mut skip = true;
    for a in &args {
        if skip {
            skip = false;
        } else if a == "-p" || a == "--bin" {
            skip = true;
        } else {
            positional.push(a.as_str());
        }
    }
    let result = match args.first().map(String::as_str) {
        Some("new") => match positional.first() {
            Some(folder) => new(Path::new(folder)),
            None => Err("where? bindery new FOLDER".into()),
        },
        Some("dev") => dev(Path::new(positional.first().copied().unwrap_or(".")), flag("-p").as_deref(), flag("--bin").as_deref(), &app_args),
        _ => {
            print!("{USAGE}");
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("bindery: {e}");
        std::process::exit(1);
    }
}

fn new(folder: &Path) -> Result<(), String> {
    let package = folder.file_name().and_then(|n| n.to_str()).ok_or(format!("{} has no name to give the app", folder.display()))?.to_string();
    if !package.starts_with(|c: char| c.is_ascii_lowercase()) || !package.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        return Err(format!("“{package}” can't name an app: use lowercase letters, digits and dashes, starting with a letter"));
    }
    if folder.exists() && std::fs::read_dir(folder).map_or(true, |mut d| d.next().is_some()) {
        return Err(format!("{} already exists and isn't empty", folder.display()));
    }
    let words: Vec<&str> = package.split(['-', '_']).filter(|w| !w.is_empty()).collect();
    let facts: String = words.iter().map(|w| capitalized(w)).collect();
    let title = capitalized(&words.join(" "));
    let manifest = match workspace(folder) {
        Some(_) => format!(
            "[package]\nname = \"{package}\"\nversion.workspace = true\nedition.workspace = true\nlicense.workspace = true\npublish = false\n\n[lints]\nworkspace = true\n\n[dependencies]\ngraticule.workspace = true\n"
        ),
        None => format!(
            "[package]\nname = \"{package}\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n\n[lints.rust]\nunsafe_code = \"forbid\"\n\n[dependencies]\ngraticule = {{ git = \"{}\" }}\n",
            env!("CARGO_PKG_REPOSITORY")
        ),
    };
    let main = TEMPLATE.replace("Hello world", &title).replace("HelloWorld", &facts);
    let src = folder.join("src");
    std::fs::create_dir_all(&src).map_err(|e| format!("{}: {e}", src.display()))?;
    std::fs::write(folder.join("Cargo.toml"), manifest).map_err(|e| format!("{}: {e}", folder.display()))?;
    std::fs::write(src.join("main.rs"), main).map_err(|e| format!("{}: {e}", src.display()))?;
    println!("Made “{title}” in {}.", folder.display());
    println!("  cargo run -p {package}                 runs it");
    println!("  cargo run -p {package} -- --specimen   opens Specimen over its named states");
    println!("  bindery dev {}   rebuilds and reloads it on every saved change", folder.display());
    if let Some(root) = workspace(folder) {
        println!("It uses the Graticule in {}; if that workspace's members don't cover {}, add it there.", root.display(), folder.display());
    }
    Ok(())
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
}

fn workspace(folder: &Path) -> Option<PathBuf> {
    let start = std::path::absolute(folder).ok()?;
    start.ancestors().skip(1).find(|dir| std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]") && t.contains("\ngraticule = "))).map(Path::to_path_buf)
}

const TEMPLATE: &str = r#"use graticule::prelude::*;

#[derive(Facts, Default)]
struct HelloWorld {
    count: u32,
}

#[derive(Commands)]
enum Cmd {
    #[label("Count")]
    Count,
    #[label("Start over")]
    StartOver,
}

impl App for HelloWorld {
    type Cmd = Cmd;

    fn available(&self, cmd: &Cmd) -> Available {
        match cmd {
            Cmd::Count => Yes,
            Cmd::StartOver => no_if(self.count == 0, t!("Nothing is counted yet")),
        }
    }

    fn update(&mut self, cmd: Cmd, _: &mut Cx<Cmd>) -> Outcome {
        match cmd {
            Cmd::Count => self.count += 1,
            Cmd::StartOver => self.count = 0,
        }
        Outcome::Changed
    }

    fn view(&self, ui: &mut Ui<Cmd>) {
        ui.page(t!("Hello world"), |ui| {
            ui.text(match self.count {
                0 => "Nothing counted yet".to_string(),
                1 => "Counted once".to_string(),
                n => format!("Counted {n} times"),
            });
            ui.row(|ui| {
                ui.button(Cmd::Count);
                ui.button(Cmd::StartOver);
            });
        });
    }

    fn states() -> States<Self> {
        States::new().add("Start", HelloWorld::default())
    }
}

fn main() {
    graticule::run::<HelloWorld>(t!("Hello world"));
}
"#;

fn dev(folder: &Path, package: Option<&str>, bin: Option<&str>, app_args: &[String]) -> Result<(), String> {
    let root = workspace_root(folder)?;
    let mut built = build(folder, package, bin)?;
    let target = built.exe.parent().and_then(Path::parent).map(Path::to_path_buf).ok_or("the build put the app nowhere Bindery knows")?;
    let copies = built.exe.parent().map(|d| d.join("bindery")).unwrap_or_default();
    let handoff = copies.join(format!("{}.handoff", built.name));
    std::fs::create_dir_all(&copies).map_err(|e| format!("{}: {e}", copies.display()))?;
    let _ = std::fs::remove_file(&handoff);
    let mut app = start(&built.exe, &copies, &handoff, app_args)?;
    let mut seen = sources(&root, &target);
    println!("bindery: running {}; it rebuilds on every saved change under {}", built.name, root.display());
    loop {
        std::thread::sleep(LOOK_EVERY);
        if let Some(child) = &mut app
            && let Ok(Some(status)) = child.try_wait()
        {
            if status.success() {
                return Ok(());
            }
            eprintln!("bindery: {} stopped ({status}); it starts again after the next change", built.name);
            app = None;
        }
        let now = sources(&root, &target);
        if now == seen {
            continue;
        }
        seen = now;
        let before = stamp(&built.exe);
        built = match build(folder, package, bin) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("bindery: {e}; the app keeps running the last build");
                continue;
            }
        };
        if app.is_some() && stamp(&built.exe) == before {
            continue;
        }
        let started = Instant::now();
        if let Some(child) = app.take() {
            goodbye(child, &built.name);
        }
        app = start(&built.exe, &copies, &handoff, app_args)?;
        println!("bindery: reloaded {} in {} ms", built.name, started.elapsed().as_millis());
    }
}

struct Built {
    exe: PathBuf,
    name: String,
}

fn workspace_root(folder: &Path) -> Result<PathBuf, String> {
    let out = Command::new("cargo").args(["locate-project", "--workspace", "--message-format", "plain"]).current_dir(folder).output().map_err(|e| format!("cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!("{} isn't in a Cargo project: {}", folder.display(), String::from_utf8_lossy(&out.stderr).trim()));
    }
    let manifest = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    manifest.parent().map(Path::to_path_buf).ok_or("cargo found no workspace".into())
}

fn build(folder: &Path, package: Option<&str>, bin: Option<&str>) -> Result<Built, String> {
    let mut cargo = Command::new("cargo");
    cargo.args(["build", "--message-format", "json-render-diagnostics"]).current_dir(folder).stdout(Stdio::piped()).stderr(Stdio::inherit());
    if let Some(p) = package {
        cargo.args(["-p", p]);
    }
    if let Some(b) = bin {
        cargo.args(["--bin", b]);
    }
    let mut child = cargo.spawn().map_err(|e| format!("cargo: {e}"))?;
    let mut exes = Vec::new();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if line.contains("\"reason\":\"compiler-artifact\"")
                && let Some(exe) = text_after(&line, "\"executable\":\"")
            {
                exes.push(PathBuf::from(exe));
            }
        }
    }
    let status = child.wait().map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err("the build failed".into());
    }
    match exes.as_slice() {
        [exe] => {
            let name = exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(Built { exe: exe.clone(), name })
        }
        [] => Err(format!("the build in {} made no program; name the app with -p", folder.display())),
        many => {
            let names: Vec<String> = many.iter().filter_map(|e| e.file_stem()).map(|s| s.to_string_lossy().into_owned()).collect();
            Err(format!("the build made {} programs; pick one with -p or --bin: {}", many.len(), names.join(", ")))
        }
    }
}

fn text_after(line: &str, key: &str) -> Option<String> {
    let rest = &line[line.find(key)? + key.len()..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

fn stamp(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn start(exe: &Path, copies: &Path, handoff: &Path, app_args: &[String]) -> Result<Option<Child>, String> {
    let copy = copies.join(exe.file_name().unwrap_or_default());
    std::fs::copy(exe, &copy).map_err(|e| format!("{}: {e}", copy.display()))?;
    let child = Command::new(&copy).args(app_args).env(RELOAD, handoff).stdin(Stdio::piped()).spawn().map_err(|e| format!("{}: {e}", copy.display()))?;
    Ok(Some(child))
}

fn goodbye(mut child: Child, name: &str) {
    if let Some(stdin) = &mut child.stdin {
        let _ = writeln!(stdin, "reload");
        let _ = stdin.flush();
    }
    let asked = Instant::now();
    while asked.elapsed() < GOODBYE {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!("bindery: {name} didn't hand over within {} s, so it was stopped", GOODBYE.as_secs());
    let _ = child.kill();
    let _ = child.wait();
}

fn sources(root: &Path, target: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') || path == target {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                folders.push(path);
            } else {
                path.hash(&mut hasher);
                meta.len().hash(&mut hasher);
                meta.modified().ok().hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}
