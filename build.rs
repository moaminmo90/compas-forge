fn main() {
    // Re-run compilation if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}