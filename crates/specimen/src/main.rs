fn main() {
    let mut args = std::env::args().skip(1);
    let ran = match args.next().as_deref() {
        Some("--render") => {
            let folder = std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()));
            let references = (args.next().as_deref() == Some("--references")).then(|| args.next()).flatten().map(std::path::PathBuf::from);
            specimen::render::<specimen::Catalog>(&folder, references.as_deref())
        }
        Some("--changes") => specimen::changes::<specimen::Catalog>(),
        Some("--approve") => specimen::approve::<specimen::Catalog>(),
        _ => kernel::shell::run(kernel::shell::Config::new("Specimen"), |waker| specimen::Specimen::alone().wake_with(waker)),
    };
    if let Err(e) = ran {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
