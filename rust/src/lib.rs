//! supersecret for Rust: credentials kept outside your repositories, in the
//! same store the Python package uses (`~/.supersecret`, one JSON file per
//! scope; see `../docs/format.md`), so either can read what the other wrote.
//!
//! ```no_run
//! let key = supersecret::get_secret("openai", "api_key")?;
//! # Ok::<(), supersecret::Error>(())
//! ```
//!
//! Compared with the Python package it never prints anything, writes each
//! scope atomically (a private temporary file in the store's folder, then a
//! rename), keeps files private (0600, the folder 0700) on Unix, refuses
//! scope names that would leave the store folder, and doesn't create files
//! when only reading.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Map, Value};

/// What can go wrong.
#[derive(Debug)]
pub enum Error {
    /// The scope has no such key.
    NoSuchSecret { scope: String, key: String },
    /// The secret isn't a string (another program stored a number, a list…).
    NotAString { scope: String, key: String },
    /// A scope or key that can't be used (see [`valid_scope`]).
    BadName(String),
    /// A scope file that isn't a JSON object.
    Corrupt { scope: String, why: String },
    /// No home folder to find `~/.supersecret` in.
    NoHome,
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoSuchSecret { scope, key } => write!(f, "'{key}' is not a secret in scope '{scope}'"),
            Error::NotAString { scope, key } => write!(f, "'{key}' in scope '{scope}' isn't a string"),
            Error::BadName(why) => write!(f, "{why}"),
            Error::Corrupt { scope, why } => write!(f, "scope '{scope}' can't be read: {why}"),
            Error::NoHome => write!(f, "no home folder (HOME or USERPROFILE) to find ~/.supersecret in"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `scope` can name a scope file: not empty, at most 255 bytes, no
/// path separators, not `.` or `..`, no NUL or control characters. (The
/// Python package takes anything; names it could write that these refuse
/// would leave the store folder.)
pub fn valid_scope(scope: &str) -> bool {
    !scope.is_empty()
        && scope.len() <= 255
        && scope != "."
        && scope != ".."
        && !scope.contains(['/', '\\'])
        && !scope.chars().any(|c| c.is_control())
}

fn check_scope(scope: &str) -> Result<()> {
    if valid_scope(scope) {
        Ok(())
    } else {
        Err(Error::BadName(format!("{scope:?} can't be a scope: give a plain name (no '/', '\\', '..' or control characters)")))
    }
}

/// The store: a folder of scope files.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// The default store: `$SUPERSECRET_DIR` if set, else `~/.supersecret`.
    pub fn default_store() -> Result<Store> {
        if let Some(d) = std::env::var_os("SUPERSECRET_DIR").filter(|d| !d.is_empty()) {
            return Ok(Store::at(d));
        }
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .or_else(|| std::env::var_os("USERPROFILE").filter(|h| !h.is_empty()))
            .ok_or(Error::NoHome)?;
        Ok(Store::at(Path::new(&home).join(".supersecret")))
    }

    /// A store in `dir` (created when something is first stored).
    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn scope_path(&self, scope: &str) -> Result<PathBuf> {
        check_scope(scope)?;
        Ok(self.dir.join(scope))
    }

    /// A scope's secrets (empty if the scope doesn't exist yet).
    fn read_scope(&self, scope: &str) -> Result<Map<String, Value>> {
        let path = self.scope_path(scope)?;
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
            Err(e) => return Err(e.into()),
        };
        if text.trim().is_empty() {
            return Ok(Map::new());
        }
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(m)) => Ok(m),
            Ok(_) => Err(Error::Corrupt { scope: scope.into(), why: "it isn't a JSON object".into() }),
            // Never include the parser's message: it can quote the file.
            Err(e) => Err(Error::Corrupt { scope: scope.into(), why: format!("invalid JSON at line {}", e.line()) }),
        }
    }

    /// Writes a scope: a private temporary file beside it, then a rename,
    /// so readers see the old file or the new one, never half of one.
    fn write_scope(&self, scope: &str, secrets: &Map<String, Value>) -> Result<()> {
        let path = self.scope_path(scope)?;
        create_private_dir(&self.dir)?;
        static N: AtomicU64 = AtomicU64::new(0);
        let tmp = self.dir.join(format!(".{scope}.tmp-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let body = serde_json::to_string(&Value::Object(secrets.clone())).expect("a map serializes");
        let result = (|| {
            let mut f = create_private_file(&tmp)?;
            f.write_all(body.as_bytes())?;
            f.sync_all()?;
            fs::rename(&tmp, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(Error::from)
    }

    /// The secret `key` in `scope`, as a string.
    pub fn get(&self, scope: &str, key: &str) -> Result<String> {
        match self.get_value(scope, key)? {
            Value::String(s) => Ok(s),
            _ => Err(Error::NotAString { scope: scope.into(), key: key.into() }),
        }
    }

    /// The secret `key` in `scope`, whatever JSON it is.
    pub fn get_value(&self, scope: &str, key: &str) -> Result<Value> {
        self.read_scope(scope)?.remove(key).ok_or_else(|| Error::NoSuchSecret { scope: scope.into(), key: key.into() })
    }

    /// Whether `scope` has `key`, without handing out its value.
    pub fn has(&self, scope: &str, key: &str) -> Result<bool> {
        Ok(self.read_scope(scope)?.contains_key(key))
    }

    /// Stores `value` as `key` in `scope`, replacing any value there.
    pub fn store(&self, scope: &str, key: &str, value: &str) -> Result<()> {
        self.store_value(scope, key, Value::String(value.into()))
    }

    pub fn store_value(&self, scope: &str, key: &str, value: Value) -> Result<()> {
        let mut secrets = self.read_scope(scope)?;
        secrets.insert(key.into(), value);
        self.write_scope(scope, &secrets)
    }

    /// Removes `key` from `scope`; whether it was there.
    pub fn remove(&self, scope: &str, key: &str) -> Result<bool> {
        let mut secrets = self.read_scope(scope)?;
        if secrets.remove(key).is_none() {
            return Ok(false);
        }
        self.write_scope(scope, &secrets)?;
        Ok(true)
    }

    /// Removes a whole scope (its file); whether it existed.
    pub fn remove_scope(&self, scope: &str) -> Result<bool> {
        match fs::remove_file(self.scope_path(scope)?) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// The scopes, sorted (temporary files left out).
    pub fn scopes(&self) -> Result<Vec<String>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.into()),
        };
        let mut out: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| !n.contains(".tmp-") && valid_scope(n))
            .collect();
        out.sort();
        Ok(out)
    }

    /// A scope's keys, sorted (never the values).
    pub fn keys(&self, scope: &str) -> Result<Vec<String>> {
        let mut keys: Vec<String> = self.read_scope(scope)?.into_iter().map(|(k, _)| k).collect();
        keys.sort();
        Ok(keys)
    }
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if dir.is_dir() {
        return Ok(());
    }
    fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)
}

#[cfg(unix)]
fn create_private_file(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn create_private_file(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().write(true).create_new(true).open(path)
}

/// `key` in `scope` from the default store ([`Store::default_store`]).
pub fn get_secret(scope: &str, key: &str) -> Result<String> {
    Store::default_store()?.get(scope, key)
}

/// Whether the default store's `scope` has `key`.
pub fn has_secret(scope: &str, key: &str) -> Result<bool> {
    Store::default_store()?.has(scope, key)
}

/// Stores `value` as `key` in `scope`, in the default store.
pub fn store_secret(scope: &str, key: &str, value: &str) -> Result<()> {
    Store::default_store()?.store(scope, key, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> Store {
        let d = std::env::temp_dir().join(format!("supersecret-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        Store::at(d.join("store"))
    }

    #[test]
    fn lifecycle() {
        let s = temp_store("life");
        assert!(!s.has("web", "user").unwrap());
        assert!(matches!(s.get("web", "user"), Err(Error::NoSuchSecret { .. })));
        assert!(!s.dir().exists(), "reading creates nothing");
        s.store("web", "user", "bob").unwrap();
        s.store("web", "pass", "hunter2").unwrap();
        assert_eq!(s.get("web", "user").unwrap(), "bob");
        assert_eq!(s.keys("web").unwrap(), ["pass", "user"]);
        assert_eq!(s.scopes().unwrap(), ["web"]);
        s.store("web", "user", "alice").unwrap();
        assert_eq!(s.get("web", "user").unwrap(), "alice");
        assert!(s.remove("web", "user").unwrap());
        assert!(!s.remove("web", "user").unwrap());
        assert_eq!(s.keys("web").unwrap(), ["pass"]);
        assert!(s.remove_scope("web").unwrap());
        assert!(s.scopes().unwrap().is_empty());
        let _ = fs::remove_dir_all(s.dir().parent().unwrap());
    }

    #[test]
    fn refuses_names_that_leave_the_store() {
        let s = temp_store("names");
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", "x\u{0}y"] {
            assert!(matches!(s.store(bad, "k", "v"), Err(Error::BadName(_))), "{bad:?}");
        }
        assert!(valid_scope("website.com"));
        assert!(valid_scope(".hidden-ok"));
    }

    #[test]
    fn reads_what_python_writes_and_keeps_other_values() {
        let s = temp_store("compat");
        fs::create_dir_all(s.dir()).unwrap();
        // json.dump's default style, with a non-string value from elsewhere.
        fs::write(s.dir().join("svc"), r#"{"token": "abc", "port": 8080}"#).unwrap();
        assert_eq!(s.get("svc", "token").unwrap(), "abc");
        assert!(matches!(s.get("svc", "port"), Err(Error::NotAString { .. })));
        s.store("svc", "token", "def").unwrap();
        let back: Value = serde_json::from_str(&fs::read_to_string(s.dir().join("svc")).unwrap()).unwrap();
        assert_eq!(back, serde_json::json!({ "token": "def", "port": 8080 }));
        // An empty file (Python's initial "{}" or nothing) is an empty scope.
        fs::write(s.dir().join("empty"), "").unwrap();
        assert!(s.keys("empty").unwrap().is_empty());
        let _ = fs::remove_dir_all(s.dir().parent().unwrap());
    }

    #[test]
    fn corrupt_scope_errors_without_quoting_it() {
        let s = temp_store("corrupt");
        fs::create_dir_all(s.dir()).unwrap();
        fs::write(s.dir().join("bad"), "{\"k\": \"s3cret").unwrap();
        let e = s.get("bad", "k").unwrap_err().to_string();
        assert!(!e.contains("s3cret"), "{e}");
        let _ = fs::remove_dir_all(s.dir().parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let s = temp_store("perms");
        s.store("p", "k", "v").unwrap();
        assert_eq!(fs::metadata(s.dir()).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(s.dir().join("p")).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(fs::read_dir(s.dir()).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().contains(".tmp-")), "no temp files left");
        let _ = fs::remove_dir_all(s.dir().parent().unwrap());
    }
}
