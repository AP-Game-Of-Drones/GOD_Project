use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // Get the project directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    // Save it into a file or an env var
    let dest_path = Path::new(&out_dir).join("build_constants.rs");
    fs::write(
        &dest_path,
        format!("pub const PROJECT_DIR: &str = {:?};", manifest_dir),
    )
    .unwrap();
}
