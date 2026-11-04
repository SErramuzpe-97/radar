use std::{path::Path, process::Command};

fn run_npm(ui_dir: &Path, args: &[&str]) {
    let status = Command::new("npm")
        .args(args)
        .current_dir(ui_dir)
        .status()
        .unwrap_or_else(|error| panic!("no se pudo ejecutar npm {:?}: {error}", args));
    assert!(status.success(), "npm {:?} terminó con {status}", args);
}

fn main() {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let ui_dir = Path::new(&manifest_dir).join("../../radar-ui");

    for input in ["src", "index.html", "package.json", "package-lock.json", "vite.config.ts", "tsconfig.json"] {
        println!("cargo:rerun-if-changed={}", ui_dir.join(input).display());
    }

    if !ui_dir.join("node_modules").exists() {
        run_npm(&ui_dir, &["ci"]);
    }
    run_npm(&ui_dir, &["run", "build"]);
}
