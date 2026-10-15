use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(not(windows))]
use crate::capitalized;
use crate::{Built, build};

const KEPT_UPDATES: usize = 10;

struct Installed {
    name: String,
    source: PathBuf,
    package: Option<String>,
    bin: Option<String>,
    commit: String,
    at: PathBuf,
}

impl Installed {
    fn line(&self) -> String {
        let or_none = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".into());
        [self.name.clone(), self.source.display().to_string(), or_none(&self.package), or_none(&self.bin), self.commit.clone(), self.at.display().to_string()].join("\t")
    }

    fn parse(line: &str) -> Option<Installed> {
        let parts: Vec<&str> = line.split('\t').collect();
        let [name, source, package, bin, commit, at] = parts.as_slice() else { return None };
        let given = |v: &str| (v != "-").then(|| v.to_string());
        Some(Installed { name: name.to_string(), source: PathBuf::from(source), package: given(package), bin: given(bin), commit: commit.to_string(), at: PathBuf::from(at) })
    }
}

fn data() -> Result<PathBuf, String> {
    dirs::data_dir().map(|d| d.join("Graticule")).ok_or_else(|| "this system names no folder for app data".to_string())
}

fn registry() -> Result<PathBuf, String> {
    Ok(data()?.join("bindery").join("installed.txt"))
}

fn installed() -> Result<Vec<Installed>, String> {
    let path = registry()?;
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(text.lines().filter_map(Installed::parse).collect())
}

fn remember(all: &[Installed]) -> Result<(), String> {
    let path = registry()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text: String = all.iter().map(|i| i.line() + "\n").collect();
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn git(source: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(source).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn commit(source: &Path) -> String {
    git(source, &["rev-parse", "HEAD"]).unwrap_or_else(|| "-".into())
}

pub fn install(folder: &Path, package: Option<&str>, bin: Option<&str>) -> Result<(), String> {
    let source = std::path::absolute(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let built = build(&source, package, bin, true)?;
    let at = place(&built)?;
    let mut all = installed()?;
    all.retain(|i| i.name != built.name);
    all.push(Installed { name: built.name.clone(), source: source.clone(), package: package.map(str::to_string), bin: bin.map(str::to_string), commit: commit(&source), at: at.clone() });
    remember(&all)?;
    println!("Installed {} at {}.", built.name, at.display());
    println!("  bindery update {}   builds it again from {} when its source moves on", built.name, source.display());
    Ok(())
}

pub fn update(name: Option<&str>, offline: bool) -> Result<(), String> {
    let mut all = installed()?;
    if all.is_empty() {
        return Err("no app is installed with Bindery yet; bindery install FOLDER installs one".into());
    }
    if let Some(name) = name
        && !all.iter().any(|i| i.name == name)
    {
        let names: Vec<&str> = all.iter().map(|i| i.name.as_str()).collect();
        return Err(format!("“{name}” isn't installed with Bindery; these are: {}", names.join(", ")));
    }
    let mut failed = 0;
    for app in all.iter_mut().filter(|i| name.is_none_or(|n| i.name == n)) {
        match update_one(app, offline) {
            Ok(said) => println!("  {}: {said}", app.name),
            Err(why) => {
                failed += 1;
                eprintln!("  {}: {why}", app.name);
            }
        }
    }
    remember(&all)?;
    match failed {
        0 => Ok(()),
        n => Err(format!("{n} of the apps couldn't be updated; each still runs the build it had")),
    }
}

fn update_one(app: &mut Installed, offline: bool) -> Result<String, String> {
    if !app.source.is_dir() {
        return Err(format!("its source, {}, is gone", app.source.display()));
    }
    if !offline && git(&app.source, &["rev-parse", "--abbrev-ref", "@{upstream}"]).is_some() {
        git(&app.source, &["pull", "--ff-only", "--quiet"]).ok_or("its source couldn't move on to the latest version without merging; bring it up to date by hand")?;
    }
    let now = commit(&app.source);
    if now == app.commit && app.at.exists() {
        return Ok("up to date".into());
    }
    let built = build(&app.source, app.package.as_deref(), app.bin.as_deref(), true)?;
    app.at = place(&built)?;
    let news = changes(&app.source, &app.commit, &now);
    tell(&app.name, &now, &news)?;
    app.commit = now;
    Ok(match news.len() {
        0 => "built again".into(),
        1 => "updated, with one change".into(),
        n => format!("updated, with {n} changes"),
    })
}

fn changes(source: &Path, from: &str, to: &str) -> Vec<String> {
    if from == "-" || to == "-" {
        return Vec::new();
    }
    git(source, &["log", "--format=%s", &format!("{from}..{to}")]).map(|log| log.lines().map(str::to_string).collect()).unwrap_or_default()
}

fn tell(name: &str, commit: &str, news: &[String]) -> Result<(), String> {
    let folder = data()?.join(name);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let path = folder.join("news.txt");
    let kept = std::fs::read_to_string(&path).unwrap_or_default();
    let earlier: Vec<&str> = kept.split("\n\n").filter(|b| !b.trim().is_empty()).take(KEPT_UPDATES - 1).collect();
    let short = &commit[..commit.len().min(7)];
    let mut block = format!("{} {short}", today());
    for line in news {
        block.push('\n');
        block.push_str(line);
    }
    let text = std::iter::once(block.as_str()).chain(earlier).collect::<Vec<_>>().join("\n\n") + "\n";
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(folder.join("news.unread"), "").map_err(|e| format!("{}: {e}", folder.display()))
}

fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    let (z, era) = (days + 719_468, (days + 719_468).div_euclid(146_097));
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(target_os = "macos")]
fn place(built: &Built) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("this system names no home folder")?;
    let title = capitalized(&built.name.replace(['-', '_'], " "));
    let bundle = home.join("Applications").join(format!("{title}.app"));
    let contents = bundle.join("Contents");
    let macos = contents.join("MacOS");
    std::fs::create_dir_all(&macos).map_err(|e| format!("{}: {e}", macos.display()))?;
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleExecutable</key>\n\t<string>{name}</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>graticule.{name}</string>\n\t<key>CFBundleName</key>\n\t<string>{title}</string>\n\t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n\t<key>NSHighResolutionCapable</key>\n\t<true/>\n</dict>\n</plist>\n",
        name = built.name
    );
    std::fs::write(contents.join("Info.plist"), plist).map_err(|e| format!("{}: {e}", contents.display()))?;
    let exe = macos.join(&built.name);
    let _ = std::fs::remove_file(&exe);
    std::fs::copy(&built.exe, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let signed = Command::new("codesign").args(["--force", "--sign", "-"]).arg(&bundle).status().map_err(|e| format!("codesign: {e}"))?;
    if !signed.success() {
        return Err(format!("codesign couldn't sign {}", bundle.display()));
    }
    let bin = home.join(".local").join("bin");
    std::fs::create_dir_all(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;
    let link = bin.join(&built.name);
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&exe, &link).map_err(|e| format!("{}: {e}", link.display()))?;
    Ok(bundle)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn place(built: &Built) -> Result<PathBuf, String> {
    let bin = dirs::executable_dir().ok_or("this system names no folder for programs")?;
    std::fs::create_dir_all(&bin).map_err(|e| format!("{}: {e}", bin.display()))?;
    let exe = bin.join(&built.name);
    let _ = std::fs::remove_file(&exe);
    std::fs::copy(&built.exe, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let title = capitalized(&built.name.replace(['-', '_'], " "));
    let entries = data()?.parent().map(|d| d.join("applications")).ok_or("this system names no folder for app data")?;
    std::fs::create_dir_all(&entries).map_err(|e| format!("{}: {e}", entries.display()))?;
    let entry = format!("[Desktop Entry]\nType=Application\nName={title}\nExec={}\nTerminal=false\n", exe.display());
    std::fs::write(entries.join(format!("{}.desktop", built.name)), entry).map_err(|e| format!("{}: {e}", entries.display()))?;
    Ok(exe)
}

#[cfg(windows)]
fn place(built: &Built) -> Result<PathBuf, String> {
    let folder = dirs::data_local_dir().ok_or("this system names no folder for programs")?.join("Programs").join("Graticule");
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let exe = folder.join(format!("{}.exe", built.name));
    let old = folder.join(format!("{}.old.exe", built.name));
    let _ = std::fs::remove_file(&old);
    if exe.exists() {
        std::fs::rename(&exe, &old).map_err(|e| format!("{}: {e}", exe.display()))?;
    }
    std::fs::copy(&built.exe, &exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    Ok(exe)
}
