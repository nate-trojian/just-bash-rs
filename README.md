# just-bash-rs

Inspired by [just-bash](https://github.com/vercel-labs/just-bash)

An in-memory bash emulator written in Rust. Provides a `Shell` struct that can execute bash-like commands with an in-memory filesystem and environment. Useful for testing shell-like behavior without touching the real filesystem.
Currently in early development; APIs may change.
This crate provides a library interface; there is no binary.

## Features

- In-memory filesystem with three modes: Memory, ReadThrough, and Passthrough
- Supports common bash commands: `ls`, `cd`, `pwd`, `cat`, `touch`, `mkdir`, `echo`, `grep`, `wc`, `rm`, `cp`, `mv`, `head`, `tail`, `find`, `sort`
- Pipes (`|`), redirection (`<`, `>`, `>>`), and semicolon-separated statements
- Variable expansion (`$VAR`, `${VAR}`), including special variables like `$?`
- Single and double quoted strings, backslash escaping
- Extensible command system

## Requirements

This library uses the Rust 2024 edition, which requires a nightly compiler.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
just-bash-rs = "0.1.0"
```

Example:

```rust
use just_bash_rs::Shell;

fn main() {
    let mut shell = Shell::new();
    let result = shell.execute("echo hello world");
    assert_eq!(result.stdout, "hello world\n");
    assert_eq!(result.exit_code, 0);
}
```

## Filesystem Modes

The shell can be created with different filesystem modes:

- `FsMode::Memory` (default): All reads and writes in memory.
- `FsMode::ReadThrough(root)`: Reads fall through to disk; writes stay in memory.
- `FsMode::Passthrough(root)`: All reads and writes go to disk.

```rust
use just_bash_rs::{Shell, FsMode};

let mut shell = Shell::with_mode(FsMode::ReadThrough("/tmp".into()));
```

## Supported Commands

| Command | Description | Example |
|---------|-------------|---------|
| `ls` | List directory contents | `ls -l /` |
| `cd` | Change directory | `cd /home` |
| `pwd` | Print working directory | `pwd` |
| `cat` | Concatenate and print files | `cat file.txt` |
| `touch` | Create empty file | `touch new.txt` |
| `mkdir` | Create directory | `mkdir -p dir/subdir` |
| `echo` | Print text | `echo -n "no newline"` |
| `grep` | Search pattern in files | `grep -i pattern file` |
| `wc` | Count lines, words, bytes | `wc -l file` |
| `rm` | Remove files/directories | `rm -rf dir` |
| `cp` | Copy files/directories | `cp -r src dst` |
| `mv` | Move/rename files | `mv old.txt new.txt` |
| `head` | Show first lines | `head -n 5 file` |
| `tail` | Show last lines | `tail -n 5 file` |
| `find` | Find files by pattern | `find / -name "*.rs"` |
| `sort` | Sort lines | `sort -n numbers.txt` |

Flags supported as per common usage (e.g., `ls -l`, `grep -i`, `sort -n`).

## Shell Methods

The `Shell` struct provides methods for execution and inspection:

- `execute(input: &str) -> ExecuteResult` – Run a shell command string.
- `env() -> &Env` – Immutable access to environment.
- `env_mut() -> &mut Env` – Mutable access to environment.
- `fs() -> &Fs` – Immutable access to filesystem.
- `fs_mut() -> &mut Fs` – Mutable access to filesystem.
- `set_var(key, value)` – Set an environment variable.
- `get_var(key) -> Option<&str>` – Get an environment variable.
- `cwd() -> &str` – Current working directory.
- `set_cwd(path)` – Change current working directory.

## Limitations

- No job control (`&`, `fg`, `bg`).
- No shell functions or aliases.
- No arithmetic expansion.
- No tilde expansion (`~`).
- Limited pattern matching (only `*` in `find`).

## Running Tests

```bash
cargo test
```

## Contributing

Contributions are welcome! Please open an issue or pull request.

## License

MIT OR Apache-2.0