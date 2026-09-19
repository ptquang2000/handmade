fn main() {
    println!("cargo::rustc-check-cfg=cfg(HANDMADE_INTERNAL)");
    println!("cargo::rustc-cfg=HANDMADE_INTERNAL");
    println!(
        "cargo::rustc-link-search=native={}/build",
        env!("CARGO_MANIFEST_DIR")
    );
    println!("cargo::rustc-link-search=native=/usr/lib/spa-0.2");
}
