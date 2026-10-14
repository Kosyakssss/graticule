fn main() {
    let config = kernel::shell::Config::new("Specimen");
    if let Err(e) = kernel::shell::run(config, |waker| specimen::Specimen::alone().wake_with(waker)) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
