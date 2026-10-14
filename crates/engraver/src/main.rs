use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "engraver FOLDER [--out FILE.rs] [--prefix PATH]

Compiles every SVG file in FOLDER into pictures and says what it made: each
picture's name, label, size and the inks of its layers, or why a file was
refused. --out also writes the Rust a build script would, with the types
under --prefix (graticule by default).";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut dir, mut out, mut prefix) = (None, None, "graticule".to_string());
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--prefix" => prefix = args.next().unwrap_or(prefix),
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if dir.is_none() => dir = Some(PathBuf::from(arg)),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        }
    }
    let Some(dir) = dir else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let pictures = match engraver::read(&dir) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let (mut code, mut names, mut refused) = (String::new(), Vec::new(), 0);
    for (file, picture) in &pictures {
        match picture {
            Ok(p) => {
                let mut inks = Vec::new();
                for l in &p.layers {
                    let ink = l.ink.to_lowercase();
                    if !inks.contains(&ink) {
                        inks.push(ink);
                    }
                }
                let layers = if p.layers.len() == 1 { "1 layer".to_string() } else { format!("{} layers", p.layers.len()) };
                let bytes: usize = p.layers.iter().map(|l| l.path.len()).sum();
                println!("{file:<24} {:<20} “{}”, {} by {}, {layers} in {}, {bytes} bytes of path", engraver::ident(&p.name), p.label, p.width, p.height, inks.join(", "));
                code.push_str(&engraver::code(p, &prefix));
                names.push(p.name.clone());
            }
            Err(why) => {
                refused += 1;
                println!("{file:<24} refused: {why}");
            }
        }
    }
    println!("{} pictures, {refused} refused", pictures.len() - refused);
    if let Some(out) = out {
        code.push_str(&engraver::all(&names, &prefix));
        if let Err(e) = std::fs::write(&out, code) {
            eprintln!("{}: {e}", out.display());
            return ExitCode::FAILURE;
        }
    }
    if refused > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
