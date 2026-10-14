fn main() {
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("icons.rs");
    if let Err(e) = engraver::engrave(std::path::Path::new("icons"), &out, "crate::picture") {
        panic!("{e}");
    }
}
