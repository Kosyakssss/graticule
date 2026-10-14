fn main() {
    let mut args = std::env::args().skip(1);
    let ran = match args.next().as_deref() {
        Some("--render") => specimen::render::<specimen::Catalog>(&std::path::PathBuf::from(args.next().unwrap_or_else(|| ".".into()))),
        _ => kernel::shell::run(kernel::shell::Config::new("Specimen"), |waker| specimen::Specimen::alone().wake_with(waker)),
    };
    if let Err(e) = ran {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
