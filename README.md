# SuperSecret

A simple library to make it easy to store credentials, retrieve credentials,
and make it so that you don't accidentally commit said credentials to your public
git repo.

```python
# from ipython
import supersecret

supersecret.storeSecret("website.com", "username", "myname")
supersecret.storeSecret("website.com", "password", "mypassword")

# From your application
import supersecret

connect_to_website(username=supersecret.getSecret("website.com", "username"),
                   password=supersecret.getSecret("website.com", "password"))
```

You can also use `python -m supersecret` to list and store supersecret.

```
$ python -m supersecret store website.com password    # asks for the value, hidden
$ python -m supersecret show website.com
```

(`store SCOPE KEY VALUE` still works, but the value then shows in your shell history.)

The secrets are stored, unencrypted, in `~/.supersecret` (or `$SUPERSECRET_DIR`).
The format, and what supersecret does and doesn't protect against, are in
[docs/format.md](docs/format.md).

## Installation

You can clone the repo and run

```shell
$ python setup.py install
```

Or you can install directly using pip and this repo

```shell
$ pip install git+git://github.com/cwgreene/supersecret.git
```

## Rust

`rust/` is a Rust crate for the same store, and a `supersecret` command:

```rust
let key = supersecret::get_secret("openai", "api_key")?;
```

```
$ supersecret store openai api_key        # hidden prompt, or piped: printf %s "$K" | supersecret store openai api_key
$ supersecret has openai api_key          # yes / no
$ supersecret keys openai                 # key names, never values
$ supersecret scopes
$ supersecret get openai api_key          # prints the value, for scripts
```

Build it with `cargo build --release` in `rust/` (the command is
`rust/target/release/supersecret`). Each reads what the other writes.

## Scopes

The first parameter in the above examples `"website.com"`
is called a scope. Each scope will correspond to a separate file in
`~/.supersecret`.

## Providers

`supersecret` allows you to use a different directory than `~/.supersecret`,
by using

```python
import supersecret

supersecret.setProvider(supersecret.FileSystemProvider("/path/to/other/directory"))
```

It is *strongly* recommended that you do not put supersecret in your repo
directory, as this would allow for accidental commits of the supersecret
(which is the main thing this library is attempting to avoid). It is
probably also recommended not to have multiple user access to this
directory.

Theoretically, you can create a new provider class; checkout `supersecret.py`
for the required functions to be implemented. I'll probably make this
interface explicit sometime with inheritance.
