# Where supersecret could go

Written 2026-11-23 with the Rust crate (SPRS-297). supersecret began as "keep API keys out
of git" (see `format.md`); agents now use it too, and the project tracker hands it values
through secret cards. These are directions worth weighing, most useful first. Each is a
Backlog task in the tracker's SPRS project.

## Near term: small, and they fix real gaps

1. **Locking.** Two programs storing into one scope at once can lose a write (last rename
   wins). Take an advisory lock (`<scope>.lock` with `flock`, or a lock file created
   exclusively) around read-modify-write, in both implementations.
2. **`supersecret run`.** `supersecret run --env OPENAI_API_KEY=openai/api_key -- python
   job.py` starts a program with secrets in its environment. Code needn't import
   supersecret at all, and an agent never sees the value: it runs the command, not `get`.
   This is the single most useful thing for agents.
3. **Python catches up with the spec.** Refuse scope names that leave the folder
   (`../x`), stop creating a file when only reading, a `keys` command that lists names
   without values, and lock (1). Tidy the packaging (version, the repository URL: the
   checkout's remote is cwgreene/secrets while the README says tessprime/supersecret).

## Medium term: stronger storage

4. **Encryption at rest.** Today anything that can read `~/.supersecret` (a backup, a
   stray `cat`, a misconfigured sync) has every secret. Encrypt scope files, e.g. with
   [age](https://age-encryption.org), keyed from the OS keychain or a passphrase cached
   by an agent process. It changes the format, so it needs a version marker and both
   implementations.
5. **OS keychain backend.** Alternatively, store values in the platform's own vault
   (macOS Keychain, Windows Credential Manager, Secret Service/libsecret on Linux),
   keeping today's files only as an index of scopes and keys. Fits machines that
   already unlock a keychain at login.
6. **Metadata and rotation.** When a secret was stored and by whom, an optional expiry,
   and a `supersecret stale` that lists what's due. This is a format change (values
   become objects), so do it with (4).

## Longer term: secrets and agents

7. **Scoped access for agents.** Today any process of the user can read any scope. A
   per-project allow-list (this project's agents may read `openai/*`), enforced by
   handing values out only through `run` or a small local daemon (like ssh-agent),
   would let one machine host agents with different access.
8. **An audit log.** Which program or agent read which key, and when. This needs reads
   to go through something that can log them (the daemon in 7, or `run`).
9. **An MCP tool.** Agents could ask for a secret (the tracker's secret card), check
   one exists, or run a command with one, through MCP without ever handling a value.
10. **Syncing between machines through the project tracker.** A secret card could store
   one value on several machines, or a "copy to another machine" could send it through
   the tracker's chat link. Worth it once values are encrypted in transit and at rest
   end to end.
11. **Redaction.** `supersecret run` could mask any stored value that the child prints,
   and the tracker could do the same when showing transcripts. It needs care: matching
   values means holding them.

## Not planned

- A server or cloud vault (HashiCorp Vault, 1Password): larger tools exist for that.
  supersecret's appeal is being a folder of files and two tiny libraries.
