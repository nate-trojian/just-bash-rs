# just-bash-rs

Inspired by [just-bash](https://github.com/vercel-labs/just-bash)

An in-memory bash emulator written in Rust. Provides a `Shell` struct that can execute bash-like commands with an in-memory filesystem and environment. Useful for testing shell-like behavior without touching the real filesystem.
Currently in early development; APIs may change.
This crate provides a library interface; there is no binary.

## Features

- In-memory filesystem with three modes: Memory, ReadThrough, and Passthrough
- 25 built-in commands with standardized argument parsing
- Pipes (`|`), redirection (`<`, `>`, `>>`), and semicolon-separated statements
- Variable expansion (`$VAR`, `${VAR}`), including special variables like `$?`
- Single and double quoted strings, backslash escaping
- Auto-generated help via `man` command
- Extensible command system with declarative metadata

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

### File & Directory Operations

| Command | Description | Example |
|---------|-------------|---------|
| `ls` | List directory contents | `ls -la /` |
| `cd` | Change directory | `cd /home` |
| `pwd` | Print working directory | `pwd` |
| `mkdir` | Create directories | `mkdir -p dir/subdir` |
| `touch` | Create empty file | `touch new.txt` |
| `cp` | Copy files/directories | `cp -r src dst` |
| `mv` | Move/rename files | `mv old.txt new.txt` |
| `rm` | Remove files/directories | `rm -rf dir` |
| `find` | Find files by pattern | `find / -name "*.rs"` |

### Text Processing

| Command | Description | Example |
|---------|-------------|---------|
| `cat` | Concatenate and print files | `cat file.txt` |
| `echo` | Print text | `echo -n "no newline"` |
| `grep` | Search pattern in files | `grep -in pattern file` |
| `wc` | Count lines, words, bytes | `wc -l file` |
| `head` | Show first lines | `head -n 5 file` |
| `tail` | Show last lines | `tail -5 file` |
| `sort` | Sort lines | `sort -nru file.txt` |
| `uniq` | Remove duplicate adjacent lines | `uniq -c file.txt` |
| `cut` | Extract fields from lines | `cut -d: -f1,3 /etc/passwd` |
| `tr` | Translate or delete characters | `echo hello \| tr a-z A-Z` |
| `sed` | Stream editor | `sed 's/old/new/g' file` |
| `diff` | Compare files line by line | `diff a.txt b.txt` |

### Utilities

| Command | Description | Example |
|---------|-------------|---------|
| `basename` | Strip directory from path | `basename /usr/bin/foo` |
| `tee` | Write to stdout and files | `echo hi \| tee out.txt` |
| `xargs` | Execute command with stdin args | `echo file.txt \| xargs wc -l` |
| `man` | Show help for a command | `man ls` |

## Command Reference

Use `man` to get detailed help for any command:

```
$ man ls
Usage: ls [-la] [path...]

List directory contents.

Options:
  -l                    Show long format (permissions, size, name)
  -a                    Show hidden files (starting with .)
```

Use `man` with no arguments to list all available commands:

```
$ man
Available commands:

  basename      Strip directory from path
  cat           Concatenate and print files
  cd            Change working directory
  ...
```

## Argument Parsing

All commands use a standardized argument parser that supports:

- **Combined flags**: `ls -la` (same as `ls -l -a`)
- **Value flags**: `head -n 10` or `head -n10`
- **Numeric shorthand**: `head -5` (equivalent to `head -n 5`)
- **Long options**: `find -name "*.rs"` (single-dash long form)
- **Stop parsing**: `grep -- -v` (treats `-v` as an argument, not a flag)

## Text Processing Details

### sed

Supports substitution, delete, and print commands with address selectors:

```
sed 's/pattern/replacement/gi'     # global, case-insensitive substitute
sed '2d'                           # delete line 2
sed '/pattern/d'                   # delete lines matching pattern
sed '2,4d'                         # delete lines 2-4
sed -n '2p'                        # print only line 2
sed -e 's/a/b/' -e 's/c/d/'       # multiple commands
sed 's/a/b/;s/c/d/'               # semicolon-separated commands
```

### tr

Supports character translation, deletion, and POSIX character classes:

```
echo hello | tr a-z A-Z           # translate lowercase to uppercase
echo hello | tr -d l              # delete 'l' characters
echo aaabbb | tr -s a             # squeeze repeated 'a'
echo hello | tr -c a-z _          # complement: non-alpha becomes _
echo hello | tr '[:lower:]' '[:upper:]'  # POSIX classes
```

Supported POSIX classes: `[:upper:]`, `[:lower:]`, `[:digit:]`, `[:alpha:]`, `[:alnum:]`, `[:space:]`, `[:blank:]`, `[:print:]`, `[:graph:]`, `[:punct:]`, `[:cntrl:]`, `[:xdigit:]`, `[:word:]`

### xargs

Reads whitespace-separated tokens from stdin and passes them as arguments:

```
echo 'a b c' | xargs              # echo a b c
echo 'file1 file2' | xargs wc -l  # wc -l file1 file2
echo 'a b c d' | xargs -n 2       # echo a b; echo c d
```

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

## Extending with New Commands

To add a command, define metadata and a handler function:

```rust
use just_bash_rs::argparse::{CommandMeta, FlagMeta, PositionalMeta, StdinBehavior, parse_args};

const META_FOO: CommandMeta = CommandMeta {
    name: "foo",
    synopsis: "foo [-v] [file...]",
    description: "Do something with files",
    details: "",
    flags: &[FlagMeta {
        short: 'v',
        long: None,
        takes_value: false,
        value_hint: "",
        description: "Verbose output",
    }],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: true,
        description: "Files to process",
    }],
    stdin: StdinBehavior::Optional,
};

fn cmd_foo(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env, exec: &PipelineExec) -> (String, String, i32) {
    let parsed = parse_args(&META_FOO, args);
    let verbose = parsed.has_flag('v');
    // ... command logic
}
```

Register in `get_commands()`:

```rust
cmds.insert("foo", (cmd_foo, &META_FOO));
```

`man foo` works automatically from the metadata.

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
