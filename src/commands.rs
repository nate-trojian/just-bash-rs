use std::collections::HashMap;

use crate::env::Env;
use crate::fs::{DirEntry, Fs};

/// Function signature for all built-in commands.
type CommandFn = fn(&[String], &str, &mut Fs, &mut Env) -> (String, String, i32);

/// Return a map of command name → handler function.
pub fn get_commands() -> HashMap<&'static str, CommandFn> {
    let mut cmds: HashMap<&'static str, CommandFn> = HashMap::new();
    cmds.insert("ls", cmd_ls);
    cmds.insert("cd", cmd_cd);
    cmds.insert("pwd", cmd_pwd);
    cmds.insert("cat", cmd_cat);
    cmds.insert("touch", cmd_touch);
    cmds.insert("mkdir", cmd_mkdir);
    cmds.insert("echo", cmd_echo);
    cmds.insert("grep", cmd_grep);
    cmds.insert("wc", cmd_wc);
    cmds.insert("rm", cmd_rm);
    cmds.insert("cp", cmd_cp);
    cmds.insert("mv", cmd_mv);
    cmds.insert("head", cmd_head);
    cmds.insert("tail", cmd_tail);
    cmds.insert("find", cmd_find);
    cmds.insert("sort", cmd_sort);
    cmds
}

// ── ls ────────────────────────────────────────────────────────────

fn cmd_ls(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut show_long = false;
    let mut show_hidden = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'l' => show_long = true,
                    'a' => show_hidden = true,
                    _ => {}
                }
            }
        } else {
            paths.push(arg.as_str());
        }
    }

    if paths.is_empty() {
        paths.push(".");
    }

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
            // Single file
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

fn cmd_cd(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let target = if args.is_empty() {
        env.get("HOME").unwrap_or("/").to_string()
    } else {
        args[0].clone()
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

fn cmd_pwd(_args: &[String], _stdin: &str, _fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    (format!("{}\n", env.cwd()), String::new(), 0)
}

// ── cat ───────────────────────────────────────────────────────────

fn cmd_cat(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
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

fn cmd_touch(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    if args.is_empty() {
        return (
            String::new(),
            "touch: missing file operand\n".to_string(),
            1,
        );
    }

    for arg in args {
        if !fs.exists(arg, env.cwd()) {
            fs.create_file(arg, env.cwd());
        }
        // If file exists, we just "touch" it (no-op for mtime in this impl)
    }

    (String::new(), String::new(), 0)
}

// ── mkdir ─────────────────────────────────────────────────────────

fn cmd_mkdir(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut parents = false;
    let mut dirs = Vec::new();

    for arg in args {
        if arg == "-p" {
            parents = true;
        } else {
            dirs.push(arg.as_str());
        }
    }

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

fn cmd_echo(args: &[String], _stdin: &str, _fs: &mut Fs, _env: &mut Env) -> (String, String, i32) {
    let mut no_newline = false;
    let mut start = 0;

    if !args.is_empty() && args[0] == "-n" {
        no_newline = true;
        start = 1;
    }

    let mut output = args[start..].join(" ");
    if !no_newline {
        output.push('\n');
    }
    (output, String::new(), 0)
}

// ── grep ──────────────────────────────────────────────────────────

fn cmd_grep(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut case_insensitive = false;
    let mut show_line_numbers = false;
    let mut pattern = None;
    let mut files = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'i' => case_insensitive = true,
                    'n' => show_line_numbers = true,
                    _ => {}
                }
            }
        } else if pattern.is_none() {
            pattern = Some(arg.clone());
        } else {
            files.push(arg.as_str());
        }
    }

    let pattern = match pattern {
        Some(mut p) => {
            // Strip surrounding quotes
            if (p.starts_with('\'') && p.ends_with('\''))
                || (p.starts_with('"') && p.ends_with('"'))
            {
                p = p[1..p.len() - 1].to_string();
            }
            p
        }
        None => return (String::new(), "grep: no pattern specified\n".to_string(), 2),
    };

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

fn cmd_wc(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut show_lines = false;
    let mut show_words = false;
    let mut show_bytes = false;
    let mut files = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'l' => show_lines = true,
                    'w' => show_words = true,
                    'c' => show_bytes = true,
                    _ => {}
                }
            }
        } else {
            files.push(arg.as_str());
        }
    }

    // If no flags, show all
    if !show_lines && !show_words && !show_bytes {
        show_lines = true;
        show_words = true;
        show_bytes = true;
    }

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
        for file in &files {
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

fn cmd_rm(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut recursive = false;
    let mut force = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'r' | 'R' => recursive = true,
                    'f' => force = true,
                    _ => {}
                }
            }
        } else {
            paths.push(arg.as_str());
        }
    }

    if paths.is_empty() {
        return (String::new(), "rm: missing operand\n".to_string(), 1);
    }

    let mut exit_code = 0;

    for path in &paths {
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

fn cmd_cp(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut recursive = false;
    let mut paths = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                if ch == 'r' || ch == 'R' {
                    recursive = true;
                }
            }
        } else {
            paths.push(arg.as_str());
        }
    }

    if paths.len() < 2 {
        return (String::new(), "cp: missing destination\n".to_string(), 1);
    }

    let src = paths[0];
    let dst = paths[1];

    if fs.is_dir(src, env.cwd()) {
        if !recursive {
            return (
                String::new(),
                format!("cp: -r not specified; omitting directory '{}'\n", src),
                1,
            );
        }
        copy_dir(fs, src, dst, env.cwd());
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

fn copy_dir(fs: &mut Fs, src: &str, dst: &str, cwd: &str) -> bool {
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
                copy_dir(fs, &src_child, &dst_child, cwd);
            } else {
                fs.copy_file(&src_child, &dst_child, cwd);
            }
        }
    }
    true
}

// ── mv ────────────────────────────────────────────────────────────

fn cmd_mv(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    if args.len() < 2 {
        return (String::new(), "mv: missing destination\n".to_string(), 1);
    }

    let src = &args[0];
    let dst = &args[1];

    if !fs.move_node(src, dst, env.cwd()) {
        return (String::new(), format!("mv: cannot move '{}'\n", src), 1);
    }

    (String::new(), String::new(), 0)
}

// ── head ──────────────────────────────────────────────────────────

fn cmd_head(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut n: usize = 10;
    let mut files = Vec::new();
    let mut i = 0;

    while i < args.len() {
        if args[i] == "-n" && i + 1 < args.len() {
            n = args[i + 1].parse().unwrap_or(10);
            i += 2;
        } else if args[i].starts_with('-') && args[i].len() > 1 {
            // -N shorthand
            if let Ok(num) = args[i][1..].parse::<usize>() {
                n = num;
            }
            i += 1;
        } else {
            files.push(args[i].as_str());
            i += 1;
        }
    }

    let text = if files.is_empty() {
        stdin.to_string()
    } else {
        match fs.read_file(files[0], env.cwd()) {
            Some(content) => String::from_utf8_lossy(&content).to_string(),
            None => {
                return (
                    String::new(),
                    format!("head: cannot open '{}'\n", files[0]),
                    1,
                )
            }
        }
    };

    let lines: Vec<&str> = text.lines().collect();
    let result: Vec<&str> = lines.into_iter().take(n).collect();
    (result.join("\n") + "\n", String::new(), 0)
}

// ── tail ──────────────────────────────────────────────────────────

fn cmd_tail(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut n: usize = 10;
    let mut files = Vec::new();
    let mut i = 0;

    while i < args.len() {
        if args[i] == "-n" && i + 1 < args.len() {
            n = args[i + 1].parse().unwrap_or(10);
            i += 2;
        } else if args[i].starts_with('-') && args[i].len() > 1 {
            if let Ok(num) = args[i][1..].parse::<usize>() {
                n = num;
            }
            i += 1;
        } else {
            files.push(args[i].as_str());
            i += 1;
        }
    }

    let text = if files.is_empty() {
        stdin.to_string()
    } else {
        match fs.read_file(files[0], env.cwd()) {
            Some(content) => String::from_utf8_lossy(&content).to_string(),
            None => {
                return (
                    String::new(),
                    format!("tail: cannot open '{}'\n", files[0]),
                    1,
                )
            }
        }
    };

    let lines: Vec<&str> = text.lines().collect();
    let skip = if lines.len() > n { lines.len() - n } else { 0 };
    let result: Vec<&str> = lines.into_iter().skip(skip).collect();
    (result.join("\n") + "\n", String::new(), 0)
}

// ── find ──────────────────────────────────────────────────────────

fn cmd_find(args: &[String], _stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut start_path = env.cwd().to_string();
    let mut pattern = None;
    let mut i = 0;

    while i < args.len() {
        if args[i] == "-name" && i + 1 < args.len() {
            pattern = Some(args[i + 1].clone());
            i += 2;
        } else if !args[i].starts_with('-') {
            start_path = args[i].clone();
            i += 1;
        } else {
            i += 1;
        }
    }

    let pattern = match pattern {
        Some(mut p) => {
            // Strip surrounding quotes
            if (p.starts_with('\'') && p.ends_with('\''))
                || (p.starts_with('"') && p.ends_with('"'))
            {
                p = p[1..p.len() - 1].to_string();
            }
            p
        }
        None => {
            // No -name, just list all files
            let results = fs.find(&start_path, "*");
            let output: String = results.into_iter().map(|p| p + "\n").collect();
            return (output, String::new(), 0);
        }
    };

    let results = fs.find(&start_path, &pattern);
    let output: String = results.into_iter().map(|p| p + "\n").collect();
    (output, String::new(), 0)
}

// ── sort ──────────────────────────────────────────────────────────

fn cmd_sort(args: &[String], stdin: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    let mut reverse = false;
    let mut numeric = false;
    let mut unique = false;
    let mut files = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            for ch in arg[1..].chars() {
                match ch {
                    'r' => reverse = true,
                    'n' => numeric = true,
                    'u' => unique = true,
                    _ => {}
                }
            }
        } else {
            files.push(arg.as_str());
        }
    }

    let text = if files.is_empty() {
        stdin.to_string()
    } else {
        match fs.read_file(files[0], env.cwd()) {
            Some(content) => String::from_utf8_lossy(&content).to_string(),
            None => {
                return (
                    String::new(),
                    format!("sort: cannot read '{}'\n", files[0]),
                    1,
                )
            }
        }
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
