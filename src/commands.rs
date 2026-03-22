use std::collections::HashMap;

use crate::argparse::{
    format_command_list, format_help, parse_args, FlagMeta, PositionalMeta, StdinBehavior,
};
// Re-export for lib.rs usage
pub use crate::argparse::CommandMeta;
use crate::env::Env;
use crate::fs::{DirEntry, Fs};

/// Callback type for executing a pipeline string (e.g. "echo hello | grep h").
/// Provided by the shell to commands that need to run other commands (like xargs).
pub type PipelineExec = dyn Fn(&str, &mut Fs, &mut Env) -> (String, String, i32);

/// Function signature for all built-in commands.
pub type CommandFn = fn(&[String], &str, &mut Fs, &mut Env, &PipelineExec) -> (String, String, i32);

// ══════════════════════════════════════════════════════════════════
// Command metadata
// ══════════════════════════════════════════════════════════════════

const META_LS: CommandMeta = CommandMeta {
    name: "ls",
    synopsis: "ls [-la] [path...]",
    description: "List directory contents",
    details: "With no arguments, lists the current directory.",
    flags: &[
        FlagMeta {
            short: 'l',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show long format (permissions, size, name)",
        },
        FlagMeta {
            short: 'a',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show hidden files (starting with .)",
        },
    ],
    positional: &[PositionalMeta {
        name: "path",
        required: false,
        variadic: true,
        description: "Files or directories to list",
    }],
    stdin: StdinBehavior::Never,
};

const META_CD: CommandMeta = CommandMeta {
    name: "cd",
    synopsis: "cd [directory]",
    description: "Change working directory",
    details: "Defaults to $HOME if no argument given. Supports ~, .., and absolute/relative paths.",
    flags: &[],
    positional: &[PositionalMeta {
        name: "directory",
        required: false,
        variadic: false,
        description: "Target directory",
    }],
    stdin: StdinBehavior::Never,
};

const META_PWD: CommandMeta = CommandMeta {
    name: "pwd",
    synopsis: "pwd",
    description: "Print working directory",
    details: "",
    flags: &[],
    positional: &[],
    stdin: StdinBehavior::Never,
};

const META_CAT: CommandMeta = CommandMeta {
    name: "cat",
    synopsis: "cat [file...]",
    description: "Concatenate and print files",
    details: "If no files given, reads from stdin.",
    flags: &[],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: true,
        description: "Files to read",
    }],
    stdin: StdinBehavior::Optional,
};

const META_TOUCH: CommandMeta = CommandMeta {
    name: "touch",
    synopsis: "touch file...",
    description: "Create empty files or update timestamps",
    details: "",
    flags: &[],
    positional: &[PositionalMeta {
        name: "file",
        required: true,
        variadic: true,
        description: "Files to create",
    }],
    stdin: StdinBehavior::Never,
};

const META_MKDIR: CommandMeta = CommandMeta {
    name: "mkdir",
    synopsis: "mkdir [-p] directory...",
    description: "Create directories",
    details: "",
    flags: &[FlagMeta {
        short: 'p',
        long: None,
        takes_value: false,
        value_hint: "",
        description: "Create parent directories as needed",
    }],
    positional: &[PositionalMeta {
        name: "directory",
        required: true,
        variadic: true,
        description: "Directories to create",
    }],
    stdin: StdinBehavior::Never,
};

const META_ECHO: CommandMeta = CommandMeta {
    name: "echo",
    synopsis: "echo [-n] [text...]",
    description: "Print text to stdout",
    details: "",
    flags: &[FlagMeta {
        short: 'n',
        long: None,
        takes_value: false,
        value_hint: "",
        description: "Do not output trailing newline",
    }],
    positional: &[PositionalMeta {
        name: "text",
        required: false,
        variadic: true,
        description: "Text to print",
    }],
    stdin: StdinBehavior::Never,
};

const META_GREP: CommandMeta = CommandMeta {
    name: "grep",
    synopsis: "grep [-in] pattern [file...]",
    description: "Search for pattern in files or stdin",
    details: "",
    flags: &[
        FlagMeta {
            short: 'i',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Case-insensitive matching",
        },
        FlagMeta {
            short: 'n',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show line numbers",
        },
    ],
    positional: &[
        PositionalMeta {
            name: "pattern",
            required: true,
            variadic: false,
            description: "Pattern to search for",
        },
        PositionalMeta {
            name: "file",
            required: false,
            variadic: true,
            description: "Files to search",
        },
    ],
    stdin: StdinBehavior::Optional,
};

const META_WC: CommandMeta = CommandMeta {
    name: "wc",
    synopsis: "wc [-lwc] [file...]",
    description: "Count lines, words, and bytes",
    details: "Default shows all three counts. If no files given, reads from stdin.",
    flags: &[
        FlagMeta {
            short: 'l',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show line count",
        },
        FlagMeta {
            short: 'w',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show word count",
        },
        FlagMeta {
            short: 'c',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show byte count",
        },
    ],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: true,
        description: "Files to count",
    }],
    stdin: StdinBehavior::Optional,
};

const META_RM: CommandMeta = CommandMeta {
    name: "rm",
    synopsis: "rm [-rf] path...",
    description: "Remove files or directories",
    details: "",
    flags: &[
        FlagMeta {
            short: 'r',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Remove directories recursively",
        },
        FlagMeta {
            short: 'f',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Force removal, ignore nonexistent files",
        },
    ],
    positional: &[PositionalMeta {
        name: "path",
        required: true,
        variadic: true,
        description: "Files or directories to remove",
    }],
    stdin: StdinBehavior::Never,
};

const META_CP: CommandMeta = CommandMeta {
    name: "cp",
    synopsis: "cp [-r] source dest",
    description: "Copy files or directories",
    details: "",
    flags: &[FlagMeta {
        short: 'r',
        long: None,
        takes_value: false,
        value_hint: "",
        description: "Copy directories recursively",
    }],
    positional: &[
        PositionalMeta {
            name: "source",
            required: true,
            variadic: false,
            description: "Source file or directory",
        },
        PositionalMeta {
            name: "dest",
            required: true,
            variadic: false,
            description: "Destination",
        },
    ],
    stdin: StdinBehavior::Never,
};

const META_MV: CommandMeta = CommandMeta {
    name: "mv",
    synopsis: "mv source dest",
    description: "Move or rename files",
    details: "",
    flags: &[],
    positional: &[
        PositionalMeta {
            name: "source",
            required: true,
            variadic: false,
            description: "Source file or directory",
        },
        PositionalMeta {
            name: "dest",
            required: true,
            variadic: false,
            description: "Destination",
        },
    ],
    stdin: StdinBehavior::Never,
};

const META_HEAD: CommandMeta = CommandMeta {
    name: "head",
    synopsis: "head [-n NUM] [file]",
    description: "Show first lines of a file",
    details: "Default is 10 lines. Supports -N shorthand (e.g. head -5).",
    flags: &[FlagMeta {
        short: 'n',
        long: None,
        takes_value: true,
        value_hint: "NUM",
        description: "Number of lines to show",
    }],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: false,
        description: "File to read",
    }],
    stdin: StdinBehavior::Optional,
};

const META_TAIL: CommandMeta = CommandMeta {
    name: "tail",
    synopsis: "tail [-n NUM] [file]",
    description: "Show last lines of a file",
    details: "Default is 10 lines. Supports -N shorthand (e.g. tail -5).",
    flags: &[FlagMeta {
        short: 'n',
        long: None,
        takes_value: true,
        value_hint: "NUM",
        description: "Number of lines to show",
    }],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: false,
        description: "File to read",
    }],
    stdin: StdinBehavior::Optional,
};

const META_FIND: CommandMeta = CommandMeta {
    name: "find",
    synopsis: "find [path] [-name pattern]",
    description: "Find files by name pattern",
    details: "Supports glob patterns (*, ?, [abc]).",
    flags: &[FlagMeta {
        short: 'n',
        long: Some("name"),
        takes_value: true,
        value_hint: "PATTERN",
        description: "Glob pattern to match filenames",
    }],
    positional: &[PositionalMeta {
        name: "path",
        required: false,
        variadic: false,
        description: "Starting directory (default: current)",
    }],
    stdin: StdinBehavior::Never,
};

const META_SORT: CommandMeta = CommandMeta {
    name: "sort",
    synopsis: "sort [-rnu] [file]",
    description: "Sort lines of text",
    details: "If no file given, reads from stdin.",
    flags: &[
        FlagMeta {
            short: 'r',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Reverse sort order",
        },
        FlagMeta {
            short: 'n',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Numeric sort",
        },
        FlagMeta {
            short: 'u',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Remove duplicate lines",
        },
    ],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: false,
        description: "File to sort",
    }],
    stdin: StdinBehavior::Optional,
};

const META_UNIQ: CommandMeta = CommandMeta {
    name: "uniq",
    synopsis: "uniq [-cdu] [file]",
    description: "Remove duplicate adjacent lines",
    details: "If no file given, reads from stdin.",
    flags: &[
        FlagMeta {
            short: 'c',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show count of each line",
        },
        FlagMeta {
            short: 'd',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show only duplicate lines",
        },
        FlagMeta {
            short: 'u',
            long: None,
            takes_value: false,
            value_hint: "",
            description: "Show only unique lines",
        },
    ],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: false,
        description: "File to process",
    }],
    stdin: StdinBehavior::Optional,
};

const META_CUT: CommandMeta = CommandMeta {
    name: "cut",
    synopsis: "cut [-d DELIM] [-f FIELDS] [file]",
    description: "Extract fields from lines",
    details:
        "FIELDS is a comma-separated list of field numbers (1-based).\nDefault delimiter is tab.",
    flags: &[
        FlagMeta {
            short: 'd',
            long: None,
            takes_value: true,
            value_hint: "DELIM",
            description: "Field delimiter (default: tab)",
        },
        FlagMeta {
            short: 'f',
            long: None,
            takes_value: true,
            value_hint: "FIELDS",
            description: "Fields to extract (e.g. 1,3,5)",
        },
    ],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: false,
        description: "File to process",
    }],
    stdin: StdinBehavior::Optional,
};

const META_TR: CommandMeta = CommandMeta {
    name: "tr",
    synopsis: "tr [-cds] set1 [set2]",
    description: "Translate or delete characters",
    details: "set1 and set2 can use ranges (a-z) and POSIX classes ([:upper:]).\nWith -d, only set1 is needed. With set2, translates characters.",
    flags: &[
        FlagMeta { short: 'c', long: None, takes_value: false, value_hint: "", description: "Complement set1 (use characters NOT in set1)" },
        FlagMeta { short: 'd', long: None, takes_value: false, value_hint: "", description: "Delete characters in set1" },
        FlagMeta { short: 's', long: None, takes_value: false, value_hint: "", description: "Squeeze repeated characters" },
    ],
    positional: &[
        PositionalMeta { name: "set1", required: true, variadic: false, description: "Input character set" },
        PositionalMeta { name: "set2", required: false, variadic: false, description: "Output character set (for translation)" },
    ],
    stdin: StdinBehavior::Required,
};

const META_SED: CommandMeta = CommandMeta {
    name: "sed",
    synopsis: "sed [-n] [-e CMD] [file]",
    description: "Stream editor for text transformation",
    details: "Supports s/pattern/replacement/[gi], d (delete), p (print) commands.\nAddresses: /pattern/, N (line number), $ (last), N,M (range).\nMultiple commands via -e or semicolons.",
    flags: &[
        FlagMeta { short: 'n', long: None, takes_value: false, value_hint: "", description: "Suppress automatic printing" },
        FlagMeta { short: 'e', long: None, takes_value: true, value_hint: "CMD", description: "Add command (multiple allowed)" },
        FlagMeta { short: 'i', long: None, takes_value: false, value_hint: "", description: "In-place editing (ignored)" },
    ],
    positional: &[PositionalMeta { name: "file", required: false, variadic: false, description: "File to process" }],
    stdin: StdinBehavior::Optional,
};

const META_BASENAME: CommandMeta = CommandMeta {
    name: "basename",
    synopsis: "basename path [suffix]",
    description: "Strip directory from path",
    details: "",
    flags: &[],
    positional: &[
        PositionalMeta {
            name: "path",
            required: true,
            variadic: false,
            description: "Path to strip",
        },
        PositionalMeta {
            name: "suffix",
            required: false,
            variadic: false,
            description: "Suffix to remove",
        },
    ],
    stdin: StdinBehavior::Never,
};

const META_TEE: CommandMeta = CommandMeta {
    name: "tee",
    synopsis: "tee [-a] [file...]",
    description: "Write stdin to stdout and files",
    details: "",
    flags: &[FlagMeta {
        short: 'a',
        long: None,
        takes_value: false,
        value_hint: "",
        description: "Append to files instead of overwriting",
    }],
    positional: &[PositionalMeta {
        name: "file",
        required: false,
        variadic: true,
        description: "Files to write to",
    }],
    stdin: StdinBehavior::Required,
};

const META_XARGS: CommandMeta = CommandMeta {
    name: "xargs",
    synopsis: "xargs [-n NUM] [command [args...]]",
    description: "Execute command with arguments from stdin",
    details: "Reads whitespace-separated tokens from stdin and passes them\nas arguments to the specified command.",
    flags: &[
        FlagMeta { short: 'n', long: None, takes_value: true, value_hint: "NUM", description: "Max arguments per invocation" },
        FlagMeta { short: '0', long: None, takes_value: false, value_hint: "", description: "Input is null-delimited" },
    ],
    positional: &[
        PositionalMeta { name: "command", required: false, variadic: false, description: "Command to execute (default: echo)" },
        PositionalMeta { name: "args", required: false, variadic: true, description: "Arguments for command" },
    ],
    stdin: StdinBehavior::Required,
};

const META_DIFF: CommandMeta = CommandMeta {
    name: "diff",
    synopsis: "diff file1 file2",
    description: "Compare files line by line",
    details: "",
    flags: &[],
    positional: &[
        PositionalMeta {
            name: "file1",
            required: true,
            variadic: false,
            description: "First file",
        },
        PositionalMeta {
            name: "file2",
            required: true,
            variadic: false,
            description: "Second file",
        },
    ],
    stdin: StdinBehavior::Never,
};

const META_MAN: CommandMeta = CommandMeta {
    name: "man",
    synopsis: "man [command]",
    description: "Show help for a command",
    details: "Without arguments, lists all available commands.",
    flags: &[],
    positional: &[PositionalMeta {
        name: "command",
        required: false,
        variadic: false,
        description: "Command to show help for",
    }],
    stdin: StdinBehavior::Never,
};

// ══════════════════════════════════════════════════════════════════
// Command registry
// ══════════════════════════════════════════════════════════════════

/// Return a map of command name → (handler, metadata).
pub fn get_commands() -> HashMap<&'static str, (CommandFn, &'static CommandMeta)> {
    let mut cmds: HashMap<&'static str, (CommandFn, &'static CommandMeta)> = HashMap::new();
    cmds.insert("ls", (cmd_ls, &META_LS));
    cmds.insert("cd", (cmd_cd, &META_CD));
    cmds.insert("pwd", (cmd_pwd, &META_PWD));
    cmds.insert("cat", (cmd_cat, &META_CAT));
    cmds.insert("touch", (cmd_touch, &META_TOUCH));
    cmds.insert("mkdir", (cmd_mkdir, &META_MKDIR));
    cmds.insert("echo", (cmd_echo, &META_ECHO));
    cmds.insert("grep", (cmd_grep, &META_GREP));
    cmds.insert("wc", (cmd_wc, &META_WC));
    cmds.insert("rm", (cmd_rm, &META_RM));
    cmds.insert("cp", (cmd_cp, &META_CP));
    cmds.insert("mv", (cmd_mv, &META_MV));
    cmds.insert("head", (cmd_head, &META_HEAD));
    cmds.insert("tail", (cmd_tail, &META_TAIL));
    cmds.insert("find", (cmd_find, &META_FIND));
    cmds.insert("sort", (cmd_sort, &META_SORT));
    cmds.insert("uniq", (cmd_uniq, &META_UNIQ));
    cmds.insert("cut", (cmd_cut, &META_CUT));
    cmds.insert("tr", (cmd_tr, &META_TR));
    cmds.insert("sed", (cmd_sed, &META_SED));
    cmds.insert("basename", (cmd_basename, &META_BASENAME));
    cmds.insert("tee", (cmd_tee, &META_TEE));
    cmds.insert("xargs", (cmd_xargs, &META_XARGS));
    cmds.insert("diff", (cmd_diff, &META_DIFF));
    cmds.insert("man", (cmd_man, &META_MAN));
    cmds
}

// ══════════════════════════════════════════════════════════════════
// Helper: get file text from stdin or first positional arg
// ══════════════════════════════════════════════════════════════════

fn get_file_text<'a>(
    parsed: &'a crate::argparse::ParsedArgs,
    stdin: &'a str,
    fs: &'a mut Fs,
    env: &'a mut Env,
) -> Result<String, (String, String, i32)> {
    if parsed.positional.is_empty() {
        Ok(stdin.to_string())
    } else {
        match fs.read_file(&parsed.positional[0], env.cwd()) {
            Some(content) => Ok(String::from_utf8_lossy(&content).to_string()),
            None => Err((
                String::new(),
                format!("cannot read '{}'\n", &parsed.positional[0]),
                1,
            )),
        }
    }
}

// ── ls ────────────────────────────────────────────────────────────

fn cmd_ls(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_LS, args);
    if !parsed.errors.is_empty() {
        return (String::new(), parsed.errors.join("\n") + "\n", 1);
    }
    let show_long = parsed.has_flag('l');
    let show_hidden = parsed.has_flag('a');

    let paths = if parsed.positional.is_empty() {
        vec!["."]
    } else {
        parsed
            .positional
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
    };

    let mut output = String::new();
    for path in &paths {
        if fs.is_dir(path, env.cwd()) {
            if paths.len() > 1 {
                output.push_str(&format!("{}:\n", path));
            }
            if let Some(entries) = fs.list_dir(path, env.cwd(), show_hidden) {
                output.push_str(&format_ls(&entries, show_long));
            }
        } else if fs.exists(path, env.cwd()) {
            if show_long {
                let name = match path.rfind('/') {
                    Some(i) => &path[i + 1..],
                    None => path,
                };
                let entry = DirEntry {
                    name: name.to_string(),
                    is_dir: false,
                    size: fs.read_file(path, env.cwd()).map_or(0, |c| c.len() as u64),
                };
                output.push_str(&format_ls_entry(&entry));
            } else {
                let name = match path.rfind('/') {
                    Some(i) => &path[i + 1..],
                    None => path,
                };
                output.push_str(name);
                output.push('\n');
            }
        } else {
            return (
                String::new(),
                format!("ls: cannot access '{}': No such file or directory\n", path),
                1,
            );
        }
    }

    (output, String::new(), 0)
}

fn format_ls(entries: &[DirEntry], long: bool) -> String {
    if long {
        entries.iter().map(format_ls_entry).collect()
    } else {
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        names.join("  ") + "\n"
    }
}

fn format_ls_entry(entry: &DirEntry) -> String {
    let kind = if entry.is_dir { 'd' } else { '-' };
    let perms = "rwxr-xr-x";
    format!("{}{}  {:>10}  {}\n", kind, perms, entry.size, entry.name)
}

// ── cd ────────────────────────────────────────────────────────────

fn cmd_cd(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_CD, args);
    let target = if parsed.positional.is_empty() {
        env.get("HOME").unwrap_or("/").to_string()
    } else {
        parsed.positional[0].clone()
    };

    let new_cwd = if target.starts_with('/') {
        target
    } else {
        let cwd = env.cwd();
        if cwd == "/" {
            format!("/{}", target)
        } else {
            format!("{}/{}", cwd, target)
        }
    };

    // Normalize by resolving
    if let Some(_node_id) = fs.resolve(&new_cwd, "/") {
        if fs.is_dir(&new_cwd, "/") {
            // Re-resolve to get normalized path
            let normalized = if new_cwd == "/" {
                "/".to_string()
            } else {
                // Normalize by stripping trailing slash and handling ..
                let parts: Vec<&str> = new_cwd[1..]
                    .split('/')
                    .filter(|p| !p.is_empty() && *p != ".")
                    .collect();
                let mut resolved = Vec::new();
                for p in &parts {
                    if *p == ".." {
                        resolved.pop();
                    } else {
                        resolved.push(*p);
                    }
                }
                if resolved.is_empty() {
                    "/".to_string()
                } else {
                    "/".to_string() + &resolved.join("/")
                }
            };
            env.set_cwd(&normalized);
            (String::new(), String::new(), 0)
        } else {
            (
                String::new(),
                format!(
                    "cd: not a directory: {}\n",
                    args.first().map_or("", |s| s.as_str())
                ),
                1,
            )
        }
    } else {
        (
            String::new(),
            format!(
                "cd: no such file or directory: {}\n",
                args.first().map_or("", |s| s.as_str())
            ),
            1,
        )
    }
}

// ── pwd ───────────────────────────────────────────────────────────

fn cmd_pwd(
    _args: &[String],
    _stdin: &str,
    _fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    (format!("{}\n", env.cwd()), String::new(), 0)
}

// ── cat ───────────────────────────────────────────────────────────

fn cmd_cat(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    if args.is_empty() {
        return (stdin.to_string(), String::new(), 0);
    }

    let mut output = String::new();
    let mut errors = String::new();
    let mut exit_code = 0;

    for arg in args {
        match fs.read_file(arg, env.cwd()) {
            Some(content) => output.push_str(&String::from_utf8_lossy(&content)),
            None => {
                if fs.is_dir(arg, env.cwd()) {
                    errors.push_str(&format!("cat: {}: Is a directory\n", arg));
                } else {
                    errors.push_str(&format!("cat: {}: No such file or directory\n", arg));
                }
                exit_code = 1;
            }
        }
    }

    (output, errors, exit_code)
}

// ── touch ─────────────────────────────────────────────────────────

fn cmd_touch(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_TOUCH, args);
    if parsed.positional.is_empty() {
        return (
            String::new(),
            "touch: missing file operand\n".to_string(),
            1,
        );
    }
    for arg in &parsed.positional {
        if !fs.exists(arg, env.cwd()) {
            fs.create_file(arg, env.cwd());
        }
    }
    (String::new(), String::new(), 0)
}

// ── mkdir ─────────────────────────────────────────────────────────

fn cmd_mkdir(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_MKDIR, args);
    let parents = parsed.has_flag('p');
    let dirs = &parsed.positional;

    if dirs.is_empty() {
        return (String::new(), "mkdir: missing operand\n".to_string(), 1);
    }

    for dir in dirs {
        if parents {
            mkdir_p(fs, dir, env.cwd());
        } else if !fs.mkdir(dir, env.cwd()) {
            if fs.exists(dir, env.cwd()) {
                return (
                    String::new(),
                    format!("mkdir: cannot create directory '{}': File exists\n", dir),
                    1,
                );
            } else {
                return (
                    String::new(),
                    format!(
                        "mkdir: cannot create directory '{}': No such file or directory\n",
                        dir
                    ),
                    1,
                );
            }
        }
    }

    (String::new(), String::new(), 0)
}

fn mkdir_p(fs: &mut Fs, path: &str, cwd: &str) {
    // Build up each component
    let abs = if path.starts_with('/') {
        path.to_string()
    } else {
        let base = if cwd == "/" {
            String::new()
        } else {
            cwd.to_string()
        };
        format!("{}/{}", base, path)
    };

    let parts: Vec<&str> = abs[1..].split('/').filter(|p| !p.is_empty()).collect();
    let mut current = String::new();
    for part in parts {
        current.push('/');
        current.push_str(part);
        if !fs.exists(&current, "/") {
            fs.mkdir(&current, "/");
        }
    }
}

// ── echo ──────────────────────────────────────────────────────────

fn cmd_echo(
    args: &[String],
    _stdin: &str,
    _fs: &mut Fs,
    _env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_ECHO, args);
    let no_newline = parsed.has_flag('n');

    let mut output = parsed.positional.join(" ");
    if !no_newline {
        output.push('\n');
    }
    (output, String::new(), 0)
}

// ── grep ──────────────────────────────────────────────────────────

fn cmd_grep(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_GREP, args);
    if !parsed.errors.is_empty() {
        return (String::new(), parsed.errors.join("\n") + "\n", 1);
    }

    let case_insensitive = parsed.has_flag('i');
    let show_line_numbers = parsed.has_flag('n');

    let pattern = match parsed.positional.first() {
        Some(p) => {
            // Strip surrounding quotes
            let p = if (p.starts_with('\'') && p.ends_with('\''))
                || (p.starts_with('"') && p.ends_with('"'))
            {
                &p[1..p.len() - 1]
            } else {
                p.as_str()
            };
            p.to_string()
        }
        None => return (String::new(), "grep: no pattern specified\n".to_string(), 2),
    };

    let files: Vec<&str> = parsed.positional[1..].iter().map(|s| s.as_str()).collect();

    let pat = if case_insensitive {
        pattern.to_lowercase()
    } else {
        pattern.clone()
    };

    let mut output = String::new();
    let mut found = false;

    if files.is_empty() {
        for (i, line) in stdin.lines().enumerate() {
            let cmp = if case_insensitive {
                line.to_lowercase()
            } else {
                line.to_string()
            };
            if cmp.contains(&pat) {
                found = true;
                if show_line_numbers {
                    output.push_str(&format!("{}:{}\n", i + 1, line));
                } else {
                    output.push_str(line);
                    output.push('\n');
                }
            }
        }
    } else {
        let multiple = files.len() > 1;
        for file in &files {
            match fs.read_file(file, env.cwd()) {
                Some(content) => {
                    let text = String::from_utf8_lossy(&content);
                    for (i, line) in text.lines().enumerate() {
                        let cmp = if case_insensitive {
                            line.to_lowercase()
                        } else {
                            line.to_string()
                        };
                        if cmp.contains(&pat) {
                            found = true;
                            if multiple {
                                if show_line_numbers {
                                    output.push_str(&format!("{}:{}:{}\n", file, i + 1, line));
                                } else {
                                    output.push_str(&format!("{}:{}\n", file, line));
                                }
                            } else if show_line_numbers {
                                output.push_str(&format!("{}:{}\n", i + 1, line));
                            } else {
                                output.push_str(line);
                                output.push('\n');
                            }
                        }
                    }
                }
                None => {
                    output.push_str(&format!("grep: {}: No such file or directory\n", file));
                }
            }
        }
    }

    (output, String::new(), if found { 0 } else { 1 })
}

// ── wc ────────────────────────────────────────────────────────────

fn cmd_wc(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_WC, args);
    let mut show_lines = parsed.has_flag('l');
    let mut show_words = parsed.has_flag('w');
    let mut show_bytes = parsed.has_flag('c');

    // If no flags, show all
    if !show_lines && !show_words && !show_bytes {
        show_lines = true;
        show_words = true;
        show_bytes = true;
    }

    let files = &parsed.positional;

    let mut output = String::new();

    if files.is_empty() {
        let (l, w, b) = count_text(stdin);
        output.push_str(&format_count(
            l, w, b, show_lines, show_words, show_bytes, None,
        ));
    } else {
        let mut total_lines: u64 = 0;
        let mut total_words: u64 = 0;
        let mut total_bytes: u64 = 0;
        for file in files {
            match fs.read_file(file, env.cwd()) {
                Some(content) => {
                    let text = String::from_utf8_lossy(&content);
                    let (l, w, b) = count_text(&text);
                    output.push_str(&format_count(
                        l,
                        w,
                        b,
                        show_lines,
                        show_words,
                        show_bytes,
                        Some(file),
                    ));
                    total_lines += l;
                    total_words += w;
                    total_bytes += b;
                }
                None => {
                    output.push_str(&format!("wc: {}: No such file or directory\n", file));
                }
            }
        }
        if files.len() > 1 {
            output.push_str(&format_count(
                total_lines,
                total_words,
                total_bytes,
                show_lines,
                show_words,
                show_bytes,
                Some("total"),
            ));
        }
    }

    (output, String::new(), 0)
}

fn count_text(text: &str) -> (u64, u64, u64) {
    let lines = text.lines().count() as u64;
    let words = text.split_whitespace().count() as u64;
    let bytes = text.len() as u64;
    (lines, words, bytes)
}

fn format_count(
    lines: u64,
    words: u64,
    bytes: u64,
    show_l: bool,
    show_w: bool,
    show_b: bool,
    name: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if show_l {
        parts.push(format!("{:>8}", lines));
    }
    if show_w {
        parts.push(format!("{:>8}", words));
    }
    if show_b {
        parts.push(format!("{:>8}", bytes));
    }
    let mut result = parts.join(" ");
    if let Some(n) = name {
        result.push_str(&format!(" {}", n));
    }
    result.push('\n');
    result
}

// ── rm ────────────────────────────────────────────────────────────

fn cmd_rm(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_RM, args);
    let recursive = parsed.has_flag('r') || parsed.has_flag('R');
    let force = parsed.has_flag('f');
    let paths = &parsed.positional;

    if paths.is_empty() {
        return (String::new(), "rm: missing operand\n".to_string(), 1);
    }

    let mut exit_code = 0;

    for path in paths {
        let ok = if recursive {
            fs.remove_all(path, env.cwd())
        } else {
            if fs.is_dir(path, env.cwd()) && !recursive {
                if !force {
                    return (
                        String::new(),
                        format!("rm: cannot remove '{}': Is a directory\n", path),
                        1,
                    );
                }
                continue;
            }
            fs.remove(path, env.cwd())
        };

        if !ok && !force {
            if !fs.exists(path, env.cwd()) {
                return (
                    String::new(),
                    format!("rm: cannot remove '{}': No such file or directory\n", path),
                    1,
                );
            }
            exit_code = 1;
        }
    }

    (String::new(), String::new(), exit_code)
}

// ── cp ────────────────────────────────────────────────────────────

fn cmd_cp(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_CP, args);
    let recursive = parsed.has_flag('r') || parsed.has_flag('R');
    let paths = &parsed.positional;

    if paths.len() < 2 {
        return (String::new(), "cp: missing destination\n".to_string(), 1);
    }

    let src = &paths[0];
    let dst = &paths[1];

    if fs.is_dir(src, env.cwd()) {
        if !recursive {
            return (
                String::new(),
                format!("cp: -r not specified; omitting directory '{}'\n", src),
                1,
            );
        }
        // Prevent copying a directory into itself
        let src_abs = fs.resolve_abs(src, env.cwd());
        let dst_abs = fs.resolve_abs(dst, env.cwd());
        if dst_abs.starts_with(&src_abs)
            && (dst_abs.len() == src_abs.len()
                || dst_abs.as_bytes().get(src_abs.len()) == Some(&b'/'))
        {
            return (
                String::new(),
                format!("cp: cannot copy '{}' into itself\n", src),
                1,
            );
        }
        copy_dir(fs, src, dst, env.cwd(), 0);
    } else {
        if !fs.copy_file(src, dst, env.cwd()) {
            return (
                String::new(),
                format!("cp: cannot copy '{}': No such file or directory\n", src),
                1,
            );
        }
    }

    (String::new(), String::new(), 0)
}

fn copy_dir(fs: &mut Fs, src: &str, dst: &str, cwd: &str, depth: usize) -> bool {
    if depth > 100 {
        return false;
    }
    // Create destination directory
    fs.mkdir(dst, cwd);

    if let Some(entries) = fs.list_dir(src, cwd, true) {
        for entry in entries {
            let src_child = if src == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", src, entry.name)
            };
            let dst_child = if dst == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", dst, entry.name)
            };

            if entry.is_dir {
                copy_dir(fs, &src_child, &dst_child, cwd, depth + 1);
            } else {
                fs.copy_file(&src_child, &dst_child, cwd);
            }
        }
    }
    true
}

// ── mv ────────────────────────────────────────────────────────────

fn cmd_mv(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_MV, args);
    if parsed.positional.len() < 2 {
        return (String::new(), "mv: missing destination\n".to_string(), 1);
    }

    let src = &parsed.positional[0];
    let dst = &parsed.positional[1];

    if !fs.move_node(src, dst, env.cwd()) {
        return (String::new(), format!("mv: cannot move '{}'\n", src), 1);
    }

    (String::new(), String::new(), 0)
}

// ── head ──────────────────────────────────────────────────────────

fn cmd_head(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_HEAD, args);
    let n: usize = parsed
        .flag_value('n')
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let text = match get_file_text(&parsed, stdin, fs, env) {
        Ok(t) => t,
        Err(e) => return (e.0, e.1.replace("cannot read", "head: cannot open"), e.2),
    };

    let lines: Vec<&str> = text.lines().collect();
    let result: Vec<&str> = lines.into_iter().take(n).collect();
    (result.join("\n") + "\n", String::new(), 0)
}

// ── tail ──────────────────────────────────────────────────────────

fn cmd_tail(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_TAIL, args);
    let n: usize = parsed
        .flag_value('n')
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let text = match get_file_text(&parsed, stdin, fs, env) {
        Ok(t) => t,
        Err(e) => return (e.0, e.1.replace("cannot read", "tail: cannot open"), e.2),
    };

    let lines: Vec<&str> = text.lines().collect();
    let skip = if lines.len() > n { lines.len() - n } else { 0 };
    let result: Vec<&str> = lines.into_iter().skip(skip).collect();
    (result.join("\n") + "\n", String::new(), 0)
}

// ── find ──────────────────────────────────────────────────────────

fn cmd_find(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_FIND, args);
    let start_path = parsed
        .positional
        .first()
        .cloned()
        .unwrap_or_else(|| env.cwd().to_string());

    let pattern = match parsed.flag_value('n') {
        Some(p) => {
            // Strip surrounding quotes
            if (p.starts_with('\'') && p.ends_with('\''))
                || (p.starts_with('"') && p.ends_with('"'))
            {
                &p[1..p.len() - 1]
            } else {
                p
            }
        }
        None => {
            // No -name, just list all files
            let results = fs.find(&start_path, "*");
            let output: String = results.into_iter().map(|p| p + "\n").collect();
            return (output, String::new(), 0);
        }
    };

    let results = fs.find(&start_path, pattern);
    let output: String = results.into_iter().map(|p| p + "\n").collect();
    (output, String::new(), 0)
}

// ── sort ──────────────────────────────────────────────────────────

fn cmd_sort(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_SORT, args);
    let reverse = parsed.has_flag('r');
    let numeric = parsed.has_flag('n');
    let unique = parsed.has_flag('u');

    let text = match get_file_text(&parsed, stdin, fs, env) {
        Ok(t) => t,
        Err(e) => return (e.0, e.1.replace("cannot read", "sort: cannot read"), e.2),
    };

    let mut lines: Vec<String> = text.lines().map(String::from).collect();

    if numeric {
        lines.sort_by(|a, b| {
            let na = a.trim().parse::<f64>().unwrap_or(f64::NAN);
            let nb = b.trim().parse::<f64>().unwrap_or(f64::NAN);
            na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        lines.sort();
    }

    if reverse {
        lines.reverse();
    }

    if unique {
        lines.dedup();
    }

    (lines.join("\n") + "\n", String::new(), 0)
}

// ── uniq ──────────────────────────────────────────────────────────

fn cmd_uniq(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_UNIQ, args);
    let show_count = parsed.has_flag('c');
    let only_duplicates = parsed.has_flag('d');
    let only_unique = parsed.has_flag('u');

    let text = match get_file_text(&parsed, stdin, fs, env) {
        Ok(t) => t,
        Err(e) => return (e.0, e.1.replace("cannot read", "uniq: cannot read"), e.2),
    };

    let mut output = String::new();
    let lines: Vec<&str> = text.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let mut count = 1;
        while i + count < lines.len() && lines[i + count] == line {
            count += 1;
        }

        let show = if only_duplicates && only_unique {
            // Both -d and -u: show nothing (same as gnu uniq)
            false
        } else if only_duplicates {
            count > 1
        } else if only_unique {
            count == 1
        } else {
            true
        };

        if show {
            if show_count {
                output.push_str(&format!("{} {}\n", count, line));
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        i += count;
    }

    (output, String::new(), 0)
}

// ── cut ───────────────────────────────────────────────────────────

fn cmd_cut(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_CUT, args);
    if !parsed.errors.is_empty() {
        return (String::new(), parsed.errors.join("\n") + "\n", 1);
    }

    let delimiter = parsed
        .flag_value('d')
        .and_then(|v| v.chars().next())
        .unwrap_or('\t');
    let fields: Vec<usize> = parsed
        .flag_value('f')
        .map(|v| v.split(',').filter_map(|s| s.parse().ok()).collect())
        .unwrap_or_default();

    if fields.is_empty() {
        return (
            String::new(),
            "cut: you must specify a list of fields with -f\n".to_string(),
            1,
        );
    }

    let text = match get_file_text(&parsed, stdin, fs, env) {
        Ok(t) => t,
        Err(e) => return (e.0, e.1.replace("cannot read", "cut: cannot read"), e.2),
    };

    let mut output = String::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split(delimiter).collect();
        let extracted: Vec<&str> = fields
            .iter()
            .filter_map(|&f| {
                if f > 0 && f <= parts.len() {
                    Some(parts[f - 1])
                } else {
                    None
                }
            })
            .collect();
        output.push_str(&extracted.join(&delimiter.to_string()));
        output.push('\n');
    }

    (output, String::new(), 0)
}

// ── tr ────────────────────────────────────────────────────────────

fn expand_tr_set(arg: &str) -> String {
    match arg {
        "[:upper:]" => "ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(),
        "[:lower:]" => "abcdefghijklmnopqrstuvwxyz".to_string(),
        "[:digit:]" => "0123456789".to_string(),
        "[:alpha:]" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(),
        "[:alnum:]" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".to_string(),
        "[:space:]" => " \t\n\r\x0b\x0c".to_string(),
        "[:blank:]" => " \t".to_string(),
        "[:print:]" => (0x20..=0x7e).map(|c| c as u8 as char).collect(),
        "[:graph:]" => (0x21..=0x7e).map(|c| c as u8 as char).collect(),
        "[:punct:]" => "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".to_string(),
        "[:cntrl:]" => {
            let mut s: String = (0x00..=0x1f).map(|c| c as u8 as char).collect();
            s.push(0x7f as u8 as char);
            s
        }
        "[:xdigit:]" => "0123456789abcdefABCDEF".to_string(),
        "[:word:]" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_".to_string(),
        _ => expand_char_set(arg),
    }
}

fn expand_char_set(arg: &str) -> String {
    let chars: Vec<char> = arg.chars().collect();
    let mut result = String::new();
    let mut i = 0;
    while i < chars.len() {
        if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i] <= chars[i + 2] {
            for c in chars[i]..=chars[i + 2] {
                result.push(c);
            }
            i += 3;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn cmd_tr(
    args: &[String],
    stdin: &str,
    _fs: &mut Fs,
    _env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let mut delete = false;
    let mut squeeze = false;
    let mut complement = false;
    let mut set1 = None;
    let mut set2 = None;

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'd' => delete = true,
                    's' => squeeze = true,
                    'c' | 'C' => complement = true,
                    _ => {}
                }
            }
        } else if set1.is_none() {
            set1 = Some(expand_tr_set(arg));
        } else if set2.is_none() {
            set2 = Some(expand_tr_set(arg));
        }
    }

    let set1 = match set1 {
        Some(s) => s,
        None => return (String::new(), "tr: missing operand\n".to_string(), 1),
    };

    // Complement: use all bytes not in set1
    let effective_set1 = if complement {
        let s1_chars: std::collections::HashSet<char> = set1.chars().collect();
        let mut s = String::new();
        for c in (0x00..=0x7f).map(|i| i as u8 as char) {
            if !s1_chars.contains(&c) {
                s.push(c);
            }
        }
        s
    } else {
        set1
    };

    let input = stdin.to_string();

    // Combined -d and -s: delete chars in set1, then squeeze remaining set1 chars
    if delete && squeeze {
        let chars_to_delete: std::collections::HashSet<char> = effective_set1.chars().collect();
        let mut output = String::new();
        for ch in input.chars() {
            if chars_to_delete.contains(&ch) {
                continue;
            }
            output.push(ch);
        }
        return (output, String::new(), 0);
    }

    if delete {
        let chars_to_delete: std::collections::HashSet<char> = effective_set1.chars().collect();
        let mut output = String::new();
        for ch in input.chars() {
            if !chars_to_delete.contains(&ch) {
                output.push(ch);
            }
        }
        return (output, String::new(), 0);
    }

    if squeeze {
        let chars_to_squeeze: std::collections::HashSet<char> = effective_set1.chars().collect();
        let mut output = String::new();
        let mut prev: Option<char> = None;
        for ch in input.chars() {
            if chars_to_squeeze.contains(&ch) && prev == Some(ch) {
                continue;
            }
            output.push(ch);
            prev = Some(ch);
        }
        return (output, String::new(), 0);
    }

    // Translate
    let s2 = match set2 {
        Some(s) => s,
        None => {
            return (
                String::new(),
                "tr: missing operand after set\n".to_string(),
                1,
            )
        }
    };
    let from: Vec<char> = effective_set1.chars().collect();
    let to: Vec<char> = s2.chars().collect();
    let mut output = String::new();
    for ch in input.chars() {
        if let Some(pos) = from.iter().position(|&c| c == ch) {
            let replacement = if pos < to.len() {
                to[pos]
            } else {
                to[to.len() - 1]
            };
            output.push(replacement);
        } else {
            output.push(ch);
        }
    }

    (output, String::new(), 0)
}

// ── sed ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum SedAddress {
    /// Match a specific line number
    Line(usize),
    /// Match lines matching a pattern (substring match)
    Pattern(String),
    /// Match lines in a range (inclusive)
    Range(Box<SedAddress>, Box<SedAddress>),
}

#[derive(Debug, Clone)]
struct RangeState {
    started: bool,
    finished: bool,
}

impl SedAddress {
    fn matches_line(&self, line_num: usize, line: &str) -> bool {
        match self {
            SedAddress::Line(n) => line_num == *n,
            SedAddress::Pattern(pat) => {
                let pat_lower = pat.to_lowercase();
                line.to_lowercase().contains(&pat_lower)
            }
            SedAddress::Range(..) => false, // handled by matches_range
        }
    }

    fn matches_with_range(
        &self,
        line_num: usize,
        line: &str,
        range_state: &mut Option<RangeState>,
    ) -> bool {
        match self {
            SedAddress::Range(start, end) => {
                let state = range_state.get_or_insert(RangeState {
                    started: false,
                    finished: false,
                });
                if !state.started {
                    if start.matches_line(line_num, line) {
                        state.started = true;
                    }
                }
                if state.started && !state.finished {
                    if end.matches_line(line_num, line) {
                        state.finished = true;
                    }
                    return true;
                }
                false
            }
            _ => self.matches_line(line_num, line),
        }
    }
}

#[derive(Debug, Clone)]
enum SedCommand {
    Substitute {
        search: String,
        replace: String,
        global: bool,
        case_insensitive: bool,
        addr: Option<SedAddress>,
    },
    Delete {
        addr: Option<SedAddress>,
    },
    Print {
        addr: Option<SedAddress>,
    },
}

fn cmd_sed(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let mut suppress_print = false;
    let mut commands_strs: Vec<String> = Vec::new();
    let mut files = Vec::new();
    let mut i = 0;

    while i < args.len() {
        if args[i] == "-n" {
            suppress_print = true;
            i += 1;
        } else if args[i] == "-e" && i + 1 < args.len() {
            commands_strs.push(args[i + 1].clone());
            i += 2;
        } else if args[i] == "-i" {
            // -i (in-place) not supported, just skip
            i += 1;
        } else if !args[i].starts_with('-') && commands_strs.is_empty() {
            // First non-flag arg is the command (no -e)
            commands_strs.push(args[i].clone());
            i += 1;
        } else {
            files.push(args[i].clone());
            i += 1;
        }
    }

    if commands_strs.is_empty() {
        return (String::new(), "sed: no command specified\n".to_string(), 1);
    }

    // Join all command strings and split on semicolons
    let full_cmd = commands_strs.join(";");
    let parts: Vec<&str> = full_cmd
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut sed_commands = Vec::new();
    for part in parts {
        match parse_sed_command(part) {
            Some(cmd) => sed_commands.push(cmd),
            None => {
                return (
                    String::new(),
                    format!("sed: invalid command: {}\n", part),
                    1,
                )
            }
        }
    }

    let text = if files.is_empty() {
        stdin.to_string()
    } else {
        match fs.read_file(&files[0], env.cwd()) {
            Some(content) => String::from_utf8_lossy(&content).to_string(),
            None => {
                return (
                    String::new(),
                    format!("sed: cannot read '{}'\n", files[0]),
                    1,
                )
            }
        }
    };

    let mut output = String::new();
    let lines: Vec<&str> = text.lines().collect();

    // Track range state per command across all lines
    let mut range_states: Vec<Option<RangeState>> = vec![None; sed_commands.len()];

    for (line_idx, &line) in lines.iter().enumerate() {
        let line_num = line_idx + 1; // 1-based
        let mut line_deleted = false;
        let mut line_printed = false;
        let mut current_line = line.to_string();

        for (cmd_idx, cmd) in sed_commands.iter().enumerate() {
            let matched = match cmd {
                SedCommand::Substitute { addr, .. } => match addr {
                    Some(a) if matches!(a, SedAddress::Range(..)) => {
                        a.matches_with_range(line_num, &current_line, &mut range_states[cmd_idx])
                    }
                    _ => addr_matches(addr, line_num, &current_line),
                },
                SedCommand::Delete { addr } => match addr {
                    Some(a) if matches!(a, SedAddress::Range(..)) => {
                        a.matches_with_range(line_num, &current_line, &mut range_states[cmd_idx])
                    }
                    _ => addr_matches(addr, line_num, &current_line),
                },
                SedCommand::Print { addr } => match addr {
                    Some(a) if matches!(a, SedAddress::Range(..)) => {
                        a.matches_with_range(line_num, &current_line, &mut range_states[cmd_idx])
                    }
                    _ => addr_matches(addr, line_num, &current_line),
                },
            };

            match cmd {
                SedCommand::Substitute {
                    search,
                    replace,
                    global,
                    case_insensitive,
                    ..
                } => {
                    if matched {
                        current_line = sed_substitute(
                            &current_line,
                            search,
                            replace,
                            *global,
                            *case_insensitive,
                        );
                    }
                }
                SedCommand::Delete { .. } => {
                    if matched {
                        line_deleted = true;
                        break;
                    }
                }
                SedCommand::Print { .. } => {
                    if matched {
                        output.push_str(&current_line);
                        output.push('\n');
                        line_printed = true;
                    }
                }
            }
        }

        if line_deleted {
            continue;
        }

        if !suppress_print && !line_printed {
            output.push_str(&current_line);
            output.push('\n');
        }
    }

    (output, String::new(), 0)
}

fn addr_matches(addr: &Option<SedAddress>, line_num: usize, line: &str) -> bool {
    match addr {
        None => true,
        Some(a) => a.matches_line(line_num, line),
    }
}

fn parse_sed_command(input: &str) -> Option<SedCommand> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;

    // Parse optional address
    let addr = parse_sed_address(&chars, &mut pos)?;

    if pos >= chars.len() {
        return None;
    }

    let cmd_char = chars[pos];
    pos += 1;

    match cmd_char {
        's' => {
            if pos >= chars.len() {
                return None;
            }
            let delim = chars[pos];
            pos += 1;
            let (search, new_pos) = read_until(&chars, pos, delim)?;
            pos = new_pos;
            let (replace, new_pos) = read_until(&chars, pos, delim)?;
            pos = new_pos;
            let mut global = false;
            let mut case_insensitive = false;
            while pos < chars.len() {
                match chars[pos] {
                    'g' => global = true,
                    'i' => case_insensitive = true,
                    'p' => {} // print flag (we handle via explicit p command)
                    _ => {}
                }
                pos += 1;
            }
            Some(SedCommand::Substitute {
                search,
                replace,
                global,
                case_insensitive,
                addr,
            })
        }
        'd' => Some(SedCommand::Delete { addr }),
        'p' => Some(SedCommand::Print { addr }),
        _ => None,
    }
}

fn parse_sed_address(chars: &[char], pos: &mut usize) -> Option<Option<SedAddress>> {
    if *pos >= chars.len() {
        return Some(None);
    }

    // Check for range: /start/,/end/ or start,end
    // First try /pattern/
    if chars[*pos] == '/' {
        *pos += 1;
        let (pat, new_pos) = read_until(chars, *pos, '/')?;
        *pos = new_pos;

        // Check if followed by comma (range)
        if *pos < chars.len() && chars[*pos] == ',' {
            *pos += 1;
            if *pos < chars.len() && chars[*pos] == '/' {
                *pos += 1;
                let (end_pat, new_pos) = read_until(chars, *pos, '/')?;
                *pos = new_pos;
                return Some(Some(SedAddress::Range(
                    Box::new(SedAddress::Pattern(pat)),
                    Box::new(SedAddress::Pattern(end_pat)),
                )));
            } else {
                // Numeric end
                let num = read_number(chars, pos);
                return Some(Some(SedAddress::Range(
                    Box::new(SedAddress::Pattern(pat)),
                    Box::new(SedAddress::Line(num)),
                )));
            }
        }

        return Some(Some(SedAddress::Pattern(pat)));
    }

    // Try numeric address
    if chars[*pos].is_ascii_digit() {
        let num = read_number(chars, pos);
        if *pos < chars.len() && chars[*pos] == ',' {
            *pos += 1;
            if *pos < chars.len() && chars[*pos] == '/' {
                *pos += 1;
                let (end_pat, new_pos) = read_until(chars, *pos, '/')?;
                *pos = new_pos;
                return Some(Some(SedAddress::Range(
                    Box::new(SedAddress::Line(num)),
                    Box::new(SedAddress::Pattern(end_pat)),
                )));
            } else {
                let end_num = read_number(chars, pos);
                return Some(Some(SedAddress::Range(
                    Box::new(SedAddress::Line(num)),
                    Box::new(SedAddress::Line(end_num)),
                )));
            }
        }
        return Some(Some(SedAddress::Line(num)));
    }

    // Dollar sign means last line
    if chars[*pos] == '$' {
        *pos += 1;
        return Some(Some(SedAddress::Line(usize::MAX)));
    }

    // No address
    Some(None)
}

fn read_until(chars: &[char], start: usize, delim: char) -> Option<(String, usize)> {
    let mut result = String::new();
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            result.push(chars[i + 1]);
            i += 2;
        } else if chars[i] == delim {
            return Some((result, i + 1));
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    // Allow missing closing delimiter at end of input
    Some((result, i))
}

fn read_number(chars: &[char], pos: &mut usize) -> usize {
    let mut s = String::new();
    while *pos < chars.len() && chars[*pos].is_ascii_digit() {
        s.push(chars[*pos]);
        *pos += 1;
    }
    s.parse().unwrap_or(0)
}

fn sed_substitute(
    line: &str,
    search: &str,
    replace: &str,
    global: bool,
    case_insensitive: bool,
) -> String {
    if search.is_empty() {
        return line.to_string();
    }

    if case_insensitive {
        let lower_line = line.to_lowercase();
        let lower_search = search.to_lowercase();
        let mut result = String::new();
        let mut pos = 0;

        while pos <= line.len() {
            if let Some(idx) = lower_line[pos..].find(&lower_search) {
                let abs = pos + idx;
                result.push_str(&line[pos..abs]);
                result.push_str(replace);
                pos = abs + search.len();
                if !global {
                    result.push_str(&line[pos..]);
                    break;
                }
            } else {
                result.push_str(&line[pos..]);
                break;
            }
        }
        result
    } else if global {
        line.replace(search, replace)
    } else {
        line.replacen(search, replace, 1)
    }
}

// ── basename ──────────────────────────────────────────────────────

fn cmd_basename(
    args: &[String],
    _stdin: &str,
    _fs: &mut Fs,
    _env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_BASENAME, args);
    if parsed.positional.is_empty() {
        return (String::new(), "basename: missing operand\n".to_string(), 1);
    }

    let path = &parsed.positional[0];
    let suffix = parsed.positional.get(1).map(|s| s.as_str());

    let name = match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    };

    let result = if let Some(suf) = suffix {
        if let Some(stripped) = name.strip_suffix(suf) {
            stripped.to_string()
        } else {
            name.to_string()
        }
    } else {
        name.to_string()
    };

    (format!("{}\n", result), String::new(), 0)
}

// ── tee ───────────────────────────────────────────────────────────

fn cmd_tee(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_TEE, args);
    let append = parsed.has_flag('a');
    let files = &parsed.positional;

    // Write to files
    for file in files {
        if append {
            if let Some(existing) = fs.read_file(file, env.cwd()) {
                let mut data = existing;
                data.extend_from_slice(stdin.as_bytes());
                fs.write_file(file, env.cwd(), &data);
            } else {
                fs.write_file(file, env.cwd(), stdin.as_bytes());
            }
        } else {
            fs.write_file(file, env.cwd(), stdin.as_bytes());
        }
    }

    // Pass through stdin as stdout
    (stdin.to_string(), String::new(), 0)
}

// ── xargs ─────────────────────────────────────────────────────────

fn cmd_xargs(
    args: &[String],
    stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_XARGS, args);
    let max_args: usize = parsed
        .flag_value('n')
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let null_delimited = parsed.has_flag('0');

    // First positional is the command name, rest are fixed args
    let (cmd_name, cmd_args) = if parsed.positional.is_empty() {
        ("echo".to_string(), Vec::<String>::new())
    } else {
        let name = parsed.positional[0].clone();
        let fixed_args = parsed.positional[1..].to_vec();
        (name, fixed_args)
    };

    // Read stdin tokens
    let tokens: Vec<String> = if null_delimited {
        stdin
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    } else {
        stdin.split_whitespace().map(String::from).collect()
    };

    if tokens.is_empty() {
        return (String::new(), String::new(), 0);
    }

    // Build batches
    let batches: Vec<Vec<String>> = if max_args > 0 && max_args < tokens.len() {
        tokens.chunks(max_args).map(|c| c.to_vec()).collect()
    } else {
        vec![tokens]
    };

    let mut output = String::new();
    let mut last_exit_code = 0;

    for batch in &batches {
        // Build the command string: cmd_name cmd_args... batch_tokens...
        let mut cmd_parts = vec![cmd_name.clone()];
        cmd_parts.extend(cmd_args.iter().cloned());
        cmd_parts.extend(batch.iter().cloned());

        // Quote args that contain spaces
        let cmd_str: String = cmd_parts
            .iter()
            .map(|arg| {
                if arg.contains(' ') {
                    format!("'{}'", arg.replace('\'', "'\\''"))
                } else {
                    arg.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let (stdout, stderr, code) = exec(&cmd_str, fs, env);
        output.push_str(&stdout);
        last_exit_code = code;
        if !stderr.is_empty() {
            output.push_str(&stderr);
        }
        if code != 0 {
            break;
        }
    }

    (output, String::new(), last_exit_code)
}

// ── diff ──────────────────────────────────────────────────────────

fn cmd_diff(
    args: &[String],
    _stdin: &str,
    fs: &mut Fs,
    env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let parsed = parse_args(&META_DIFF, args);
    if parsed.positional.len() < 2 {
        return (String::new(), "diff: missing operand\n".to_string(), 2);
    }

    let file_a = &parsed.positional[0];
    let file_b = &parsed.positional[1];

    let text_a = match fs.read_file(file_a, env.cwd()) {
        Some(content) => String::from_utf8_lossy(&content).to_string(),
        None => {
            return (
                String::new(),
                format!("diff: {}: No such file or directory\n", file_a),
                2,
            )
        }
    };

    let text_b = match fs.read_file(file_b, env.cwd()) {
        Some(content) => String::from_utf8_lossy(&content).to_string(),
        None => {
            return (
                String::new(),
                format!("diff: {}: No such file or directory\n", file_b),
                2,
            )
        }
    };

    let lines_a: Vec<&str> = text_a.lines().collect();
    let lines_b: Vec<&str> = text_b.lines().collect();

    const MAX_DIFF_LINES: usize = 100_000;
    if lines_a.len() > MAX_DIFF_LINES || lines_b.len() > MAX_DIFF_LINES {
        return (String::new(), "diff: input too large\n".to_string(), 2);
    }

    if lines_a == lines_b {
        return (String::new(), String::new(), 0);
    }

    // Simple LCS-based diff
    let lcs = lcs(&lines_a, &lines_b);
    let mut output = String::new();
    let mut i = 0;
    let mut j = 0;
    let mut lcs_idx = 0;

    while i < lines_a.len() || j < lines_b.len() {
        if lcs_idx < lcs.len()
            && i < lines_a.len()
            && j < lines_b.len()
            && lines_a[i] == lcs[lcs_idx]
            && lines_b[j] == lcs[lcs_idx]
        {
            i += 1;
            j += 1;
            lcs_idx += 1;
        } else {
            let mut removed = Vec::new();
            while i < lines_a.len() && (lcs_idx >= lcs.len() || lines_a[i] != lcs[lcs_idx]) {
                removed.push(i);
                i += 1;
            }
            let mut added = Vec::new();
            while j < lines_b.len() && (lcs_idx >= lcs.len() || lines_b[j] != lcs[lcs_idx]) {
                added.push(j);
                j += 1;
            }

            if !removed.is_empty() && !added.is_empty() {
                let a_start = removed[0] + 1;
                let a_end = removed[removed.len() - 1] + 1;
                let b_start = added[0] + 1;
                let b_end = added[added.len() - 1] + 1;
                if a_start == a_end && b_start == b_end {
                    output.push_str(&format!("{}c{}\n", a_start, b_start));
                } else if a_start == a_end {
                    output.push_str(&format!("{}c{},{}\n", a_start, b_start, b_end));
                } else if b_start == b_end {
                    output.push_str(&format!("{},{}c{}\n", a_start, a_end, b_start));
                } else {
                    output.push_str(&format!("{},{}c{},{}\n", a_start, a_end, b_start, b_end));
                }
                for &idx in &removed {
                    output.push_str(&format!("< {}\n", lines_a[idx]));
                }
                output.push_str("---\n");
                for &idx in &added {
                    output.push_str(&format!("> {}\n", lines_b[idx]));
                }
            } else if !removed.is_empty() {
                let start = removed[0] + 1;
                let end = removed[removed.len() - 1] + 1;
                if start == end {
                    output.push_str(&format!("{}d{}\n", start, if j > 0 { j } else { 0 }));
                } else {
                    output.push_str(&format!(
                        "{},{}d{}\n",
                        start,
                        end,
                        if j > 0 { j } else { 0 }
                    ));
                }
                for &idx in &removed {
                    output.push_str(&format!("< {}\n", lines_a[idx]));
                }
            } else if !added.is_empty() {
                let start = added[0] + 1;
                let end = added[added.len() - 1] + 1;
                if start == end {
                    output.push_str(&format!("{}a{}\n", if i > 0 { i } else { 0 }, start));
                } else {
                    output.push_str(&format!(
                        "{}a{},{}\n",
                        if i > 0 { i } else { 0 },
                        start,
                        end
                    ));
                }
                for &idx in &added {
                    output.push_str(&format!("> {}\n", lines_b[idx]));
                }
            }
        }
    }

    (output, String::new(), 1)
}

fn lcs<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
    let mut table = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..a.len() {
        for j in 0..b.len() {
            if a[i] == b[j] {
                table[i + 1][j + 1] = table[i][j] + 1;
            } else {
                table[i + 1][j + 1] = table[i][j + 1].max(table[i + 1][j]);
            }
        }
    }

    // Backtrack
    let mut result = Vec::new();
    let mut i = a.len();
    let mut j = b.len();
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push(a[i - 1]);
            i -= 1;
            j -= 1;
        } else if table[i - 1][j] > table[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

// ── man ──────────────────────────────────────────────────────────

fn cmd_man(
    args: &[String],
    _stdin: &str,
    _fs: &mut Fs,
    _env: &mut Env,
    _exec: &PipelineExec,
) -> (String, String, i32) {
    let commands = get_commands();

    if args.is_empty() {
        // List all commands
        let list: Vec<(&str, &CommandMeta)> = commands
            .iter()
            .map(|(name, (_, meta))| (*name, *meta))
            .collect();
        return (format_command_list(&list), String::new(), 0);
    }

    let cmd_name = &args[0];
    match commands.get(cmd_name.as_str()) {
        Some((_, meta)) => (format_help(meta), String::new(), 0),
        None => (
            String::new(),
            format!("man: {}: command not found\n", cmd_name),
            1,
        ),
    }
}
