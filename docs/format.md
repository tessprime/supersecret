# The supersecret store

What every implementation (the Python package in `supersecret/`, the Rust crate in
`rust/`) reads and writes, so each can use the other's secrets. Rust's
`tests/python_compat.rs` checks that they do.

## Why it exists, and what it protects against

supersecret was made to keep credentials (API keys, passwords) **out of git
repositories**: code says `getSecret("website.com", "password")`, and the value lives in
the user's home folder, where no `git add` reaches it. That is its original threat
model, and the one the format is built around. Other processes running as the same
user, the values in a program's memory, and output printed to a terminal were out of
scope.

Since 2026 agents (Claude Code sessions) use it too, and that adds one concern: **what a
command prints ends up in the agent's transcript**, which is stored and shown elsewhere
(e.g. the project tracker's conversation view). So the implementations now never print a
value unless asked to (`get` in the CLI), errors never quote a scope file, and values are
never passed as command-line arguments (shell history and `ps` show those). Values in
memory, and other processes of the same user, remain out of scope. See `roadmap.md`
for where this could go next (encryption at rest, per-agent access).

## Location

The store is a folder:

1. the folder a program names explicitly (Python `FileSystemProvider(path)` with
   `setProvider`, Rust `Store::at(path)`, CLI `--dir`), else
2. `$SUPERSECRET_DIR`, if set and not empty, else
3. `~/.supersecret` (on Windows, `%USERPROFILE%\.supersecret`).

New folders are created private (0700 on Unix).

## Scopes

Each scope is one file in the folder, named exactly as the scope (`website.com`,
`openai`). Implementations should refuse names that aren't a single plain file name:
empty, `.`, `..`, containing `/` or `\`, or control characters (Rust does; Python
doesn't check yet). File names containing `.tmp-` are temporary files, not scopes.

The file is a JSON object mapping keys to values:

```json
{"username": "bob", "password": "hunter2"}
```

Values are normally strings. Other JSON values (numbers, lists) may appear and must be
kept when another key is written; a reader asking for a string may refuse them. An
empty or missing file is an empty scope (Python writes `{}` when it first reads a
scope; Rust creates nothing when only reading).

## Writing

To change a scope, read it, change the object, and replace the file atomically: write
the whole object to a new temporary file **in the store folder** (named
`.<scope>.tmp-<something>`, created private: 0600 on Unix), then rename it over the
scope's file. Readers then see the old file or the new one, never part of one. The
temporary file stays in the same folder so the rename can't become a copy across
filesystems (`/tmp` often is another one).

Two programs writing the same scope at the same moment can still lose one write (last
rename wins); there's no locking yet.
