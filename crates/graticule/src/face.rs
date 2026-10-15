use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use kernel::command::{Commands, Form};
use kernel::runner::Runner;
use ledger::{Field, Value};

const HELP: &str = "\
--ledger [history | replay N | verify | compare FOLDER | export FILE | stopped] [--from FOLDER]
    history          every change kept in the app's command log, numbered, with when
    replay N         the facts as they were after the first N changes
    verify           replays the whole log and checks it gives the facts the app saved
    compare FOLDER   the facts saved here against those saved in another folder,
                     such as a copy from another device, and where they differ
    export FILE      writes the command log to FILE as text, one change a line
    stopped          the last session that ended without the app closing, replayed
                     to see whether it fails again and where
    --from FOLDER    reads the store in FOLDER instead of this app's own
";

/// Ledger's face: shows, replays, verifies, compares and exports the facts an app keeps, from its own command line.
pub fn ledger<A: kernel::App + Default>(args: &[String]) -> Result<(), String> {
    let from = args.iter().position(|a| a == "--from").and_then(|i| args.get(i + 1)).map(PathBuf::from).unwrap_or_else(kernel::runner::folder);
    let words: Vec<&str> = args.iter().map(String::as_str).take_while(|a| *a != "--from").collect();
    let entries = || ledger::entries(&from).map_err(|e| format!("{}: {e}", from.join("log").display()));
    match words.as_slice() {
        [] | ["history"] => {
            let entries = entries()?;
            print!("{}", history::<A>(&entries));
            Ok(())
        }
        ["replay", n] => {
            let n: usize = n.parse().map_err(|_| format!("“{n}” isn't a number of changes"))?;
            let entries = entries()?;
            let mut runner = Runner::new(A::default());
            let whole = runner.replay(&entries[..n.min(entries.len())]);
            print!("{}", tree(&runner.facts.value()));
            if !whole {
                println!("Some of these changes couldn't be replayed whole, so the facts may differ from what the app showed.");
            }
            Ok(())
        }
        ["verify"] => {
            let entries = entries()?;
            let mut runner = Runner::new(A::default());
            let whole = runner.replay(&entries);
            let saved = ledger::read(&from, A::default, A::migrations()).map_err(|e| format!("{}: {e}", from.display()))?;
            let differences = differ(&saved.value(), &runner.facts.value(), "facts");
            match (whole, differences.is_empty()) {
                (true, true) => println!("Replaying the {} changes in the log gives exactly the facts the app saved.", entries.len()),
                _ => {
                    println!("Replaying the {} changes in the log doesn't give the facts the app saved:", entries.len());
                    for d in differences.iter().take(20) {
                        println!("  {d}");
                    }
                    if !whole {
                        println!("  and some changes couldn't be replayed whole");
                    }
                    return Err("the log and the saved facts disagree".into());
                }
            }
            Ok(())
        }
        ["compare", other] => {
            let other = Path::new(other);
            let here = ledger::read(&from, A::default, A::migrations()).map_err(|e| format!("{}: {e}", from.display()))?;
            let there = ledger::read(other, A::default, A::migrations()).map_err(|e| format!("{}: {e}", other.display()))?;
            let differences = differ(&here.value(), &there.value(), "facts");
            if differences.is_empty() {
                println!("Both hold the same facts.");
            } else {
                println!("They differ in {} places:", differences.len());
                for d in differences.iter().take(50) {
                    println!("  {d}");
                }
            }
            Ok(())
        }
        ["export", file] => {
            let entries = entries()?;
            let mut out = String::new();
            for (n, entry) in entries.iter().enumerate() {
                let _ = writeln!(out, "{}\t{entry}", n + 1);
            }
            std::fs::write(file, out).map_err(|e| format!("{file}: {e}"))?;
            println!("Wrote {} changes to {file}.", entries.len());
            Ok(())
        }
        ["stopped"] => {
            let session = ledger::stopped_session(&from).map_err(|e| format!("{}: {e}", from.display()))?;
            match session {
                Some(session) => print!("{}", stopped::<A>(&session)),
                None => println!("Every session so far ended with the app closing."),
            }
            Ok(())
        }
        _ => Err(HELP.into()),
    }
}

fn stopped<A: kernel::App + Default>(session: &ledger::Stopped) -> String {
    let mut out = String::new();
    let _ = match &session.said {
        Some(said) => writeln!(out, "The session that began {} ended without the app closing. The last failure it noted was “{said}”.", moment(session.began)),
        None => writeln!(out, "The session that began {} ended without the app closing, and noted no failure.", moment(session.began)),
    };
    let mut runner = Runner::new(A::default());
    runner.replay(&session.before);
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut failed = None;
    let _ = writeln!(out, "It made {} changes:", session.entries.len());
    for (n, entry) in session.entries.iter().enumerate() {
        let what = said::<A>(entry, &runner.facts);
        if !what.is_empty() {
            let _ = writeln!(out, "{:>6}  {what}", n + 1);
        }
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runner.replay(std::slice::from_ref(entry));
            runner.build();
            runner.failure().map(str::to_string)
        }));
        let why = match ran {
            Ok(None) => continue,
            Ok(Some(why)) => why,
            Err(panic) => panic.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| panic.downcast_ref::<String>().cloned()).unwrap_or_default(),
        };
        failed = Some((n + 1, why));
        break;
    }
    std::panic::set_hook(hook);
    let _ = match failed {
        Some((n, why)) => writeln!(out, "Replaying them fails again at change {n}: {why}"),
        None => writeln!(
            out,
            "Replaying them runs every change and builds every screen without failing, so the app likely stopped for a reason outside its commands and view: drawing, the window or the system."
        ),
    };
    out
}

fn history<A: kernel::App + Default>(entries: &[Value]) -> String {
    let mut out = String::new();
    let mut runner = Runner::new(A::default());
    for (n, entry) in entries.iter().enumerate() {
        let when = match entry.field("now") {
            Some(Value::Int(ms)) => moment(*ms),
            _ => String::new(),
        };
        let what = said::<A>(entry, &runner.facts);
        if !what.is_empty() {
            let _ = writeln!(out, "{:>6}  {when}  {what}", n + 1);
        }
        runner.replay(std::slice::from_ref(entry));
    }
    if out.is_empty() {
        out.push_str("The command log is empty.\n");
    }
    out
}

fn said<A: kernel::App>(entry: &Value, facts: &A) -> String {
    match entry.field("do") {
        Some(Value::Str(what)) if what == "command" => {
            let cmds: Vec<A::Cmd> = match entry.field("commands") {
                Some(Value::List(cmds)) => cmds.iter().filter_map(A::Cmd::from_value).collect(),
                _ => Vec::new(),
            };
            let Some(first) = cmds.first() else { return "a command this build doesn't know".into() };
            let mut label = String::new();
            first.label(facts, Form::Long, &mut label);
            match cmds.len() {
                1 => label,
                n => format!("{label}, and {} more", n - 1),
            }
        }
        Some(Value::Str(what)) if what == "undo" => "Undo".into(),
        Some(Value::Str(what)) if what == "redo" => "Redo".into(),
        Some(Value::Str(what)) if what == "stop" => "Stopped a change on many items".into(),
        Some(Value::Str(what)) if what == "more" => "Went on with a change on many items".into(),
        _ => String::new(),
    }
}

fn moment(ms: i64) -> String {
    let day = kernel::days::Day(ms.div_euclid(86_400_000) as i32);
    let seconds = ms.rem_euclid(86_400_000) / 1000;
    format!("{day} {:02}:{:02}:{:02} UTC", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

fn tree(value: &Value) -> String {
    let lines = kernel::scope::facts_lines(value, usize::MAX);
    let mut out = String::new();
    for line in lines.lines() {
        let mut f = line.split('\t');
        if f.next() != Some("fact") {
            continue;
        }
        let depth: usize = f.next().and_then(|d| d.parse().ok()).unwrap_or(0);
        let (name, value) = (f.next().unwrap_or_default(), f.next().unwrap_or_default());
        let _ = writeln!(out, "{}{name}{}{value}", "  ".repeat(depth), if value.is_empty() { "" } else { ": " });
    }
    out
}

/// Where two values differ, each said as the path to it and the two sides.
fn differ(a: &Value, b: &Value, at: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(a, b, at, &mut out);
    out
}

fn walk(a: &Value, b: &Value, at: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Map(x), Value::Map(y)) => {
            for (name, v) in x {
                match y.iter().find(|(n, _)| n == name) {
                    Some((_, w)) => walk(v, w, &format!("{at}.{name}"), out),
                    None => out.push(format!("{at}.{name} is only on the first side")),
                }
            }
            for (name, _) in y.iter().filter(|(n, _)| !x.iter().any(|(m, _)| m == n)) {
                out.push(format!("{at}.{name} is only on the second side"));
            }
        }
        (Value::List(x), Value::List(y)) => {
            if x.len() != y.len() {
                out.push(format!("{at} holds {} items on one side and {} on the other", x.len(), y.len()));
            }
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                walk(v, w, &format!("{at}[{}]", i + 1), out);
            }
        }
        (Value::Some(v), Value::Some(w)) => walk(v, w, at, out),
        (Value::Variant(n, v), Value::Variant(m, w)) if n == m => walk(v, w, at, out),
        _ if a == b => {}
        _ => out.push(format!("{at} is {a} on one side and {b} on the other")),
    }
}
