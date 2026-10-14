fn main() {
    let ran = specimen::offscreen::<specimen::Catalog>(std::env::args().skip(1)).unwrap_or_else(|| kernel::shell::run(kernel::shell::Config::new("Specimen"), |waker| specimen::Specimen::alone().wake_with(waker)));
    if let Err(e) = ran {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
