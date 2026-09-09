use std::error::Error;

mod build_vendored;

fn main() -> Result<(), Box<dyn Error>> {
    // Invalidate the built crate whenever these files change
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_vendored.rs");

    build_vendored::build_libdbus()?;
    Ok(())
}
