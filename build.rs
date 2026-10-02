// Transpiles `scripts/*.ox` into Rust at build time and exposes the result
// as an include-able module, so the game brain in oxidized is compiled into
// the binary with no runtime dependency on the `oxidized` binary.
//
// The `fn native` declarations in the .ox files are implemented in this
// crate (see `src/natives.rs`); the generated code calls them through
// `crate::__oxidized_natives`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let scripts = manifest.join("scripts");

    let mut generated = String::new();
    let mut names: Vec<String> = Vec::new();

    let mut entries: Vec<PathBuf> = std::fs::read_dir(&scripts)
        .expect("scripts/ directory missing")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "ox"))
        .collect();
    entries.sort();

    for entry in entries {
        let stem = entry.file_stem().unwrap().to_str().unwrap().to_string();
        let rs_path = out_dir.join(format!("{stem}.rs"));

        // The `oxidized` binary: an explicit OXIDIZED env var wins, else
        // look next to the workspace then on PATH.
        let exe = std::env::var("OXIDIZED").unwrap_or_else(|_| "oxidized".into());

        println!("cargo:rerun-if-changed={}", entry.display());
        let out = Command::new(&exe)
            .arg("build")
            .arg(&entry)
            .arg("-o")
            .arg(&rs_path)
            .output()
            .unwrap_or_else(|e| panic!("failed to run `{exe}` on {}: {e}", entry.display()));

        if !out.status.success() {
            panic!(
                "oxidized failed on {}:\n{}",
                entry.display(),
                String::from_utf8_lossy(&out.stderr)
            );
        }

        let code = std::fs::read_to_string(&rs_path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", rs_path.display()));

        // Split the generated file: the runtime + program body becomes a
        // module, so `main()` inside it can't collide with the binary's own
        // `main()`. Rename it to `ox_main` and make the program's functions
        // `pub` so `main.rs` can call them.
        let (pre_main, main_part) = split_at_main(&code);

        let body = make_pub(pre_main);
        let main_body = make_pub(main_part);
        let main_body = rename_main(&main_body);

        generated.push_str(&format!(
            "pub mod {stem} {{\n#[allow(unused_imports, dead_code, unused_variables, unused_mut, unused_parens)]\nuse crate::__oxidized_natives::*;\n{body}\n{main_body}\n}}\n\n"
        ));
        names.push(stem);
    }

    let manifest_rs = out_dir.join("ox_modules.rs");
    std::fs::write(&manifest_rs, &generated)
        .unwrap_or_else(|e| panic!("writing ox_modules.rs: {e}"));

    // `include!` the generated file rather than `mod` + a path, so the
    // dependency tracking above is all that matters.
    let include = out_dir.join("ox_generated.rs");
    std::fs::write(&include, format!("pub mod ox_modules {{\n{generated}\n}}\n"))
    .unwrap_or_else(|e| panic!("writing ox_generated.rs: {e}"));

    // Only re-run this script when the .ox files change, not on every
    // unrelated source edit.
    println!("cargo:rerun-if-changed=scripts");

    embed_icon();
}

fn embed_icon() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let icon: PathBuf = Path::new("icon.ico").into();
    if !icon.exists() {
        return;
    }
    let mut res = winres::WindowsResource::new();
    let _ = res.set_icon("icon.ico");
    let _ = res.compile();
}

/// Splits generated Rust at the program's `fn main(`, returning
/// `(everything_before, main_and_after)`.
fn split_at_main(code: &str) -> (&str, &str) {
    // A line-anchored search avoids matching a call to a user function
    // literally named `main` elsewhere.
    let mut offset = 0usize;
    for line in code.lines() {
        if line.starts_with("fn main(") {
            return (&code[..offset], &code[offset..]);
        }
        offset += line.len() + 1;
    }
    (code, "\nfn main() {}\n")
}

/// Makes every generated top-level `fn` public so the host crate can call
/// the game's logic functions.
fn make_pub(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for line in code.lines() {
        // Only `fn <lowercase-name>(` — NOT the runtime helpers, which are
        // named `fn__ox_...` and must keep their exact spelling (turning
        // them into `pub fn__ox_...` is a syntax error, not a visibility
        // change).
        if let Some(rest) = line.strip_prefix("fn ") {
            if rest.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
                out.push_str("pub fn ");
                out.push_str(rest);
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// A module can't contain `fn main`, so the program entry point is renamed.
fn rename_main(code: &str) -> String {
    code.replacen("fn main(", "fn ox_main(", 1)
}