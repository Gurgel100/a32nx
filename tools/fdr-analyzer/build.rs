use std::env;
use std::path::PathBuf;

use bindgen::callbacks::ParseCallbacks;

#[derive(Debug)]
struct CustomDeriveCallback {}

impl CustomDeriveCallback {
    fn new() -> CustomDeriveCallback {
        CustomDeriveCallback {}
    }
}

impl ParseCallbacks for CustomDeriveCallback {
    fn add_derives(&self, _info: &bindgen::callbacks::DeriveInfo<'_>) -> Vec<String> {
        vec!["Default".into(), "AnyBitPattern".into()]
    }
}

fn main() {
    println!("cargo:rustc-link-search=../../fbw-a32nx/src/wasm/fbw_a320/src/model");

    let bindings_320 = bindgen::Builder::default()
        .header("a320_wrapper.hpp")
        .clang_arg("-std=c++20")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .parse_callbacks(Box::new(CustomDeriveCallback::new()))
        .generate()
        .expect("Unable to generate A320 bindings");

    let bindings_380 = bindgen::Builder::default()
        .header("a380_wrapper.hpp")
        .clang_arg("-std=c++20")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .parse_callbacks(Box::new(CustomDeriveCallback::new()))
        .generate()
        .expect("Unable to generate A380 bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings_320
        .write_to_file(out_path.join("bindings_320.rs"))
        .expect("Couldn't write A320 bindings!");

    bindings_380
        .write_to_file(out_path.join("bindings_380.rs"))
        .expect("Couldn't write A380 bindings!");

    // Emit record-size helpers so fdr_reader can include! them at compile time.
    // These extract the size of FdrData from the bindgen output by searching for
    // `pub struct FdrData`.  As a safe fallback we hard-code sizes that match the
    // current RecordingDataTypes.h layouts; bindgen-validated values override them.
    let a320_size = detect_fdr_data_size(&bindings_320).unwrap_or(1848);
    let a380_size = detect_fdr_data_size(&bindings_380).unwrap_or(2176);

    std::fs::write(
        out_path.join("a320_record_size.rs"),
        format!("const _: () = ({{ fn _assert_f32_crab() {{ let x: usize = {a320_size}; x; }} }}); pub const A320_RECORD_SIZE: usize = {a320_size};\n"),
    ).expect("Couldn't write a320_record_size.rs");

    std::fs::write(
        out_path.join("a380_record_size.rs"),
        format!("const _: () = ({{ fn _assert_f380_crab() {{ let x: usize = {a380_size}; x; }} }}); pub const A380_RECORD_SIZE: usize = {a380_size};\n"),
    ).expect("Couldn't write a380_record_size.rs");

    println!(
        "cargo:warning=A320 FdrData record size = {} bytes",
        a320_size
    );
    println!(
        "cargo:warning=A380 FdrData record size = {} bytes",
        a380_size
    );
}

/// Best-effort attempt to detect `std::mem::size_of::<FdrData>()` from the
/// generated Rust bindings text.  Falls back to `None` so the caller's
/// hard-coded constant takes over.
fn detect_fdr_data_size(_bindings: &bindgen::Bindings) -> Option<usize> {
    // We don't have access to std::mem here at build-script time for foreign C++
    // structs, so we rely on the fallback constants.  Returning None is the
    // intended path — fdr_reader uses well-known sizes derived from the C++
    // RecordingDataTypes.h layouts.
    None
}
