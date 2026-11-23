//! `supersecret`: the store from a terminal. Values are never taken as
//! arguments (shell history and `ps` would show them): `store` reads the
//! value from a hidden prompt, or from stdin when it's piped.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::process::ExitCode;

use supersecret::Store;

const USAGE: &str = "usage: supersecret [--dir DIR] COMMAND

  store SCOPE KEY        store a secret: typed at a hidden prompt, or piped in
                         (printf %s \"$VALUE\" | supersecret store SCOPE KEY)
  has SCOPE KEY          whether it's stored (prints yes or no; exit status 0 or 1)
  get SCOPE KEY          print the value, for scripts: TOKEN=\"$(supersecret get SCOPE KEY)\".
                         Don't run it where output is recorded (agents' transcripts, CI logs)
  keys SCOPE             a scope's keys (never the values)
  scopes                 every scope
  remove SCOPE KEY       remove a secret
  remove-scope SCOPE     remove a whole scope

The store is --dir, else $SUPERSECRET_DIR, else ~/.supersecret, shared with the Python
package (import supersecret; supersecret.getSecret(SCOPE, KEY)).";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let store = match args.iter().position(|a| a == "--dir") {
        Some(i) if i + 1 < args.len() => {
            let dir = args.remove(i + 1);
            args.remove(i);
            Store::at(dir)
        }
        Some(_) => return fail("--dir needs a folder"),
        None => match Store::default_store() {
            Ok(s) => s,
            Err(e) => return fail(&e.to_string()),
        },
    };
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match a.as_slice() {
        ["store", scope, _] if !supersecret::valid_scope(scope) => Err(format!("{scope:?} can't be a scope: give a plain name")),
        ["store", scope, key] => read_value(scope, key).and_then(|v| store.store(scope, key, &v).map_err(|e| e.to_string())).map(|()| {
            eprintln!("Stored {scope}/{key} in {}.", store.dir().display());
            0
        }),
        ["store", _, _, _, ..] => Err("give the value at the prompt or on stdin, not as an argument (shell history and ps would show it)".into()),
        ["has", scope, key] => store.has(scope, key).map_err(|e| e.to_string()).map(|h| {
            println!("{}", if h { "yes" } else { "no" });
            if h { 0 } else { 1 }
        }),
        ["get", scope, key] => store.get(scope, key).map_err(|e| e.to_string()).map(|v| {
            print!("{v}");
            if std::io::stdout().is_terminal() {
                println!();
            }
            0
        }),
        ["keys", scope] => store.keys(scope).map_err(|e| e.to_string()).map(|ks| {
            ks.iter().for_each(|k| println!("{k}"));
            0
        }),
        ["scopes"] => store.scopes().map_err(|e| e.to_string()).map(|ss| {
            ss.iter().for_each(|s| println!("{s}"));
            0
        }),
        ["remove", scope, key] => store.remove(scope, key).map_err(|e| e.to_string()).map(|r| {
            eprintln!("{}", if r { format!("Removed {scope}/{key}.") } else { format!("{scope}/{key} wasn't stored.") });
            if r { 0 } else { 1 }
        }),
        ["remove-scope", scope] => store.remove_scope(scope).map_err(|e| e.to_string()).map(|r| {
            eprintln!("{}", if r { format!("Removed the scope {scope}.") } else { format!("There's no scope {scope}.") });
            if r { 0 } else { 1 }
        }),
        ["-h" | "--help" | "help"] => {
            println!("{USAGE}");
            Ok(0)
        }
        ["--version" | "version"] => {
            println!("supersecret {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        _ => {
            eprintln!("{USAGE}");
            Ok(2)
        }
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => fail(&e),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("supersecret: {msg}");
    ExitCode::from(2)
}

/// The value to store: from a hidden prompt at a terminal (typed twice),
/// else all of stdin (one trailing newline dropped).
fn read_value(scope: &str, key: &str) -> Result<String, String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        let mut v = String::new();
        stdin.lock().read_to_string(&mut v).map_err(|e| format!("reading stdin: {e}"))?;
        let v = v.strip_suffix('\n').map(|v| v.strip_suffix('\r').unwrap_or(v)).unwrap_or(&v).to_string();
        return if v.is_empty() { Err("nothing on stdin to store".into()) } else { Ok(v) };
    }
    let first = prompt_hidden(&format!("Value for {scope}/{key}: "))?;
    if first.is_empty() {
        return Err("nothing typed; nothing stored".into());
    }
    let again = prompt_hidden("Again: ")?;
    if first != again {
        return Err("the two didn't match; nothing stored".into());
    }
    Ok(first)
}

/// A line read without echoing it (Unix); elsewhere it shows as typed.
fn prompt_hidden(prompt: &str) -> Result<String, String> {
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    let _guard = EchoOff::new();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
    drop(_guard);
    eprintln!();
    Ok(line.trim_end_matches(['\n', '\r']).to_string())
}

/// Turns the terminal's echo off until dropped.
struct EchoOff {
    #[cfg(unix)]
    saved: Option<libc::termios>,
}

impl EchoOff {
    #[cfg(unix)]
    fn new() -> EchoOff {
        // SAFETY: tcgetattr/tcsetattr on stdin with a zeroed termios we own.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) != 0 {
                return EchoOff { saved: None };
            }
            let saved = t;
            t.c_lflag &= !libc::ECHO;
            libc::tcsetattr(0, libc::TCSANOW, &t);
            EchoOff { saved: Some(saved) }
        }
    }

    #[cfg(not(unix))]
    fn new() -> EchoOff {
        EchoOff {}
    }
}

impl Drop for EchoOff {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(t) = self.saved {
            // SAFETY: restores the settings read in new().
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &t);
            }
        }
    }
}
