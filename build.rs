use std::process::Command;

fn git(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .output()
        .expect("git is required to identify the build");
    assert!(output.status.success(), "git build identification failed");
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}

fn main() {
    for path in ["HEAD", "index", "refs/heads"] {
        println!(
            "cargo:rerun-if-changed={}",
            git(&["rev-parse", "--git-path", path])
        );
    }
    for path in [
        "src",
        "tests",
        "scripts",
        "docs",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "README.md",
        "AGENTS.md",
        ".gitignore",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let commit = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain", "--untracked-files=normal"]).is_empty();
    println!("cargo:rustc-env=HOORI_COMMIT={commit}");
    println!("cargo:rustc-env=HOORI_DIRTY={dirty}");
    println!(
        "cargo:rustc-env=HOORI_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
