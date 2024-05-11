fn main() {
    // cc::Build::new()
    //     .file(concat!("src/core/compress/lz4/third_party/lib/", "lz4.c"))
    //     .include("src/core/compress/lz4/third_party/lib/")
    //     .compile("lib_third_party_compress_lz4")
    compile_tty_and_generate_bindings();
}

fn compile_tty_and_generate_bindings() {
    let manifest_root = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let tty_source_dir = format!("{manifest_root}/src/toy/pty/c");
    let tty_bindings_out_dir = format!("{}/pty_bindings.rs", std::env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed={tty_source_dir}");

    cc::Build::new()
        .file(format!("{tty_source_dir}/pty_impl.c"))
        .include(&tty_source_dir)
        .warnings(false)
        .compile("pty");

    bindgen::builder()
        .header(format!("{tty_source_dir}/pty_impl.h"))
        .layout_tests(false)
        .generate()
        .expect("Failed to generate tty bindings")
        .write_to_file(&tty_bindings_out_dir)
        .expect(&format!("Failed to write bindings {tty_bindings_out_dir}"));
}
