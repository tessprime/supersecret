//! The Rust and Python implementations share one store: each reads what the
//! other writes. Runs the Python package from this repository (../supersecret)
//! with python3, in a temporary store; skipped where python3 isn't available.

use std::process::Command;

use supersecret::Store;

fn python(dir: &std::path::Path, code: &str, args: &[&str]) -> Option<String> {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let out = Command::new("python3")
        .arg("-c")
        .arg(code)
        .args(args)
        .env("PYTHONPATH", repo)
        .env("SUPERSECRET_DIR", dir)
        .output()
        .ok()?;
    assert!(out.status.success(), "python failed: {}", String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[test]
fn rust_and_python_read_each_other() {
    let dir = std::env::temp_dir().join(format!("supersecret-compat-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::at(&dir);

    // Rust writes, Python reads (and has).
    store.store("svc", "token", "from-rust ✓ \"quoted\"").unwrap();
    let Some(read) = python(&dir, "import sys, supersecret; sys.stdout.write(supersecret.getSecret('svc', 'token'))", &[]) else {
        eprintln!("python3 not available: skipped");
        return;
    };
    assert_eq!(read, "from-rust ✓ \"quoted\"");
    assert_eq!(python(&dir, "import supersecret; print(supersecret.hasSecret('svc', 'token'))", &[]).unwrap().trim(), "True");

    // Python writes (keeping Rust's key), Rust reads; and storing prints nothing.
    let out = python(&dir, "import supersecret; supersecret.storeSecret('svc', 'other', 'from-python')", &[]).unwrap();
    assert_eq!(out, "", "storeSecret mustn't print the scope's secrets");
    assert_eq!(store.get("svc", "other").unwrap(), "from-python");
    assert_eq!(store.get("svc", "token").unwrap(), "from-rust ✓ \"quoted\"");
    assert_eq!(store.keys("svc").unwrap(), ["other", "token"]);
    let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains(".tmp-")).collect();
    assert!(leftovers.is_empty(), "no temporary files left behind");
    let _ = std::fs::remove_dir_all(&dir);
}
