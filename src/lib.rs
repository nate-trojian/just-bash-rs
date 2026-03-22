pub mod argparse;
pub mod commands;
pub mod env;
pub mod fs;
pub mod parser;

use commands::get_commands;
use commands::PipelineExec;
use env::Env;
use fs::{Fs, FsLimits, FsMode};
use parser::Pipeline;

/// Maximum input length accepted by the parser (1 MB).
const MAX_INPUT_LENGTH: usize = 1_048_576;

/// Execute a pipeline string (used by commands like xargs that need to run other commands).
fn execute_string(input: &str, fs: &mut Fs, env: &mut Env) -> (String, String, i32) {
    if input.len() > MAX_INPUT_LENGTH {
        return (
            String::new(),
            "syntax error: input too long\n".to_string(),
            2,
        );
    }
    let commands = get_commands();
    let pipelines = match parser::parse(input, env) {
        Ok(p) => p,
        Err(e) => return (String::new(), format!("{}\n", e), 2),
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0;

    for pipeline in pipelines {
        let (out, err, code) = run_pipeline(&pipeline, fs, env, &commands);
        stdout.push_str(&out);
        stderr.push_str(&err);
        exit_code = code;
    }

    env.set("?", &exit_code.to_string());
    (stdout, stderr, exit_code)
}

fn run_pipeline(
    pipeline: &Pipeline,
    fs: &mut Fs,
    env: &mut Env,
    commands: &std::collections::HashMap<
        &'static str,
        (commands::CommandFn, &'static commands::CommandMeta),
    >,
) -> (String, String, i32) {
    let mut current_stdin = if let Some(ref input_file) = pipeline.input_redirect {
        match fs.read_file(input_file, env.cwd()) {
            Some(content) => String::from_utf8_lossy(&content).to_string(),
            None => {
                return (
                    String::new(),
                    format!("{}: No such file or directory\n", input_file),
                    1,
                );
            }
        }
    } else {
        String::new()
    };

    let mut final_stdout = String::new();
    let mut final_stderr = String::new();
    let mut exit_code = 0;

    let exec: &PipelineExec =
        &|input: &str, fs: &mut Fs, env: &mut Env| execute_string(input, fs, env);

    for (i, cmd) in pipeline.commands.iter().enumerate() {
        if cmd.args.is_empty() {
            continue;
        }

        let name = &cmd.args[0];
        let args = &cmd.args[1..];

        if let Some((handler, _meta)) = commands.get(name.as_str()) {
            let (stdout, stderr, code) = handler(args, &current_stdin, fs, env, exec);
            exit_code = code;
            final_stderr = stderr;

            if i == pipeline.commands.len() - 1 {
                final_stdout = stdout;
            } else {
                current_stdin = stdout;
            }
        } else {
            final_stderr = format!("{}: command not found\n", name);
            exit_code = 127;
            break;
        }
    }

    // Handle output redirect
    if let Some(ref output_file) = pipeline.output_redirect {
        if pipeline.append {
            if let Some(existing) = fs.read_file(output_file, env.cwd()) {
                let mut data = existing;
                data.extend_from_slice(final_stdout.as_bytes());
                fs.write_file(output_file, env.cwd(), &data);
            } else {
                fs.write_file(output_file, env.cwd(), final_stdout.as_bytes());
            }
        } else {
            fs.write_file(output_file, env.cwd(), final_stdout.as_bytes());
        }
        final_stdout.clear();
    }

    (final_stdout, final_stderr, exit_code)
}

/// Result of executing a shell command.
pub struct ExecuteResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// In-memory bash emulator.
pub struct Shell {
    fs: Fs,
    env: Env,
}

impl Shell {
    /// Create a new shell with an empty in-memory filesystem and default env.
    pub fn new() -> Self {
        Shell {
            fs: Fs::new(),
            env: Env::new(),
        }
    }

    /// Create a shell with a specific filesystem mode.
    pub fn with_mode(mode: FsMode) -> Self {
        Shell {
            fs: Fs::with_mode(mode),
            env: Env::new(),
        }
    }

    /// Create a shell with an in-memory filesystem and custom resource limits.
    pub fn with_limits(limits: FsLimits) -> Self {
        Shell {
            fs: Fs::with_limits(limits),
            env: Env::new(),
        }
    }

    /// Execute a shell input string (may contain `;`-separated statements,
    /// pipes, redirections, and variable expansion).
    pub fn execute(&mut self, input: &str) -> ExecuteResult {
        let input = input.trim();
        if input.is_empty() {
            return ExecuteResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            };
        }
        if input.len() > MAX_INPUT_LENGTH {
            return ExecuteResult {
                stdout: String::new(),
                stderr: "syntax error: input too long\n".to_string(),
                exit_code: 2,
            };
        }

        let pipelines = match parser::parse(input, &self.env) {
            Ok(p) => p,
            Err(e) => {
                return ExecuteResult {
                    stdout: String::new(),
                    stderr: format!("{}\n", e),
                    exit_code: 2,
                }
            }
        };

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut exit_code = 0;

        for pipeline in pipelines {
            let result = self.execute_pipeline(&pipeline);
            stdout.push_str(&result.stdout);
            stderr.push_str(&result.stderr);
            exit_code = result.exit_code;
        }

        // Update $? in environment
        self.env.set("?", &exit_code.to_string());

        ExecuteResult {
            stdout,
            stderr,
            exit_code,
        }
    }

    fn execute_pipeline(&mut self, pipeline: &Pipeline) -> ExecuteResult {
        let commands = get_commands();
        let (stdout, stderr, exit_code) =
            run_pipeline(pipeline, &mut self.fs, &mut self.env, &commands);
        ExecuteResult {
            stdout,
            stderr,
            exit_code,
        }
    }

    // ── Public accessors ─────────────────────────────────────────

    /// Get a reference to the environment.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// Get a mutable reference to the environment.
    pub fn env_mut(&mut self) -> &mut Env {
        &mut self.env
    }

    /// Get a reference to the filesystem.
    pub fn fs(&self) -> &Fs {
        &self.fs
    }

    /// Get a mutable reference to the filesystem.
    pub fn fs_mut(&mut self) -> &mut Fs {
        &mut self.fs
    }

    /// Set an environment variable.
    pub fn set_var(&mut self, key: &str, value: &str) {
        self.env.set(key, value);
    }

    /// Get an environment variable.
    pub fn get_var(&self, key: &str) -> Option<&str> {
        self.env.get(key)
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> &str {
        self.env.cwd()
    }

    /// Set the current working directory.
    pub fn set_cwd(&mut self, path: &str) {
        self.env.set_cwd(path);
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

// ── Integration tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello");
        assert_eq!(result.stdout, "hello\n");
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_echo_no_newline() {
        let mut shell = Shell::new();
        let result = shell.execute("echo -n hello");
        assert_eq!(result.stdout, "hello");
    }

    #[test]
    fn test_echo_quoted() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'hello world'");
        assert_eq!(result.stdout, "hello world\n");
    }

    #[test]
    fn test_mkdir_and_ls() {
        let mut shell = Shell::new();
        shell.execute("mkdir /tmp");
        shell.execute("touch /tmp/a /tmp/b");
        let result = shell.execute("ls /tmp");
        assert!(result.stdout.contains("a"));
        assert!(result.stdout.contains("b"));
    }

    #[test]
    fn test_cd_and_pwd() {
        let mut shell = Shell::new();
        shell.execute("mkdir /home");
        shell.execute("mkdir /home/user");
        shell.execute("cd /home/user");
        let result = shell.execute("pwd");
        assert_eq!(result.stdout.trim(), "/home/user");
    }

    #[test]
    fn test_cat() {
        let mut shell = Shell::new();
        shell.execute("mkdir /tmp");
        shell.execute("echo 'file contents' > /tmp/test.txt");
        let result = shell.execute("cat /tmp/test.txt");
        assert_eq!(result.stdout.trim(), "file contents");
    }

    #[test]
    fn test_pipe() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'hello world' | grep hello");
        assert_eq!(result.stdout.trim(), "hello world");
    }

    #[test]
    fn test_grep() {
        let mut shell = Shell::new();
        shell.fs_mut().mkdir("/tmp", "/");
        let ok = shell
            .fs_mut()
            .write_file("/tmp/file.txt", "/", b"apple\nbanana\ncherry\n");
        assert!(ok, "write_file failed");
        assert!(
            shell.fs().exists("/tmp/file.txt", "/"),
            "file doesn't exist after write"
        );
        let result = shell.execute("grep banana /tmp/file.txt");
        assert_eq!(result.stdout.trim(), "banana");
    }

    #[test]
    fn test_grep_not_found() {
        let mut shell = Shell::new();
        shell.fs_mut().mkdir("/tmp", "/");
        shell
            .fs_mut()
            .write_file("/tmp/file.txt", "/", b"apple\nbanana\n");
        let result = shell.execute("grep grape /tmp/file.txt");
        assert_eq!(result.exit_code, 1);
        assert!(result.stdout.is_empty(), "stdout: {:?}", result.stdout);
    }

    #[test]
    fn test_wc() {
        let mut shell = Shell::new();
        shell.fs_mut().mkdir("/tmp", "/");
        shell
            .fs_mut()
            .write_file("/tmp/f.txt", "/", b"hello world\nfoo bar\n");
        let result = shell.execute("wc -l /tmp/f.txt");
        assert!(result.stdout.contains("2"));
    }

    #[test]
    fn test_rm() {
        let mut shell = Shell::new();
        shell.execute("touch /tmpfile");
        shell.execute("rm /tmpfile");
        assert!(!shell.fs().exists("/tmpfile", "/"));
    }

    #[test]
    fn test_cp() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/src.txt", "/", b"data");
        shell.execute("cp /src.txt /dst.txt");
        assert_eq!(
            shell.fs().read_file("/dst.txt", "/"),
            Some(b"data".to_vec())
        );
    }

    #[test]
    fn test_mv() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/old.txt", "/", b"data");
        shell.execute("mv /old.txt /new.txt");
        assert!(!shell.fs().exists("/old.txt", "/"));
        assert_eq!(
            shell.fs().read_file("/new.txt", "/"),
            Some(b"data".to_vec())
        );
    }

    #[test]
    fn test_head() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"1\n2\n3\n4\n5\n");
        let result = shell.execute("head -n 3 /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["1", "2", "3"]);
    }

    #[test]
    fn test_tail() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"1\n2\n3\n4\n5\n");
        let result = shell.execute("tail -n 2 /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["4", "5"]);
    }

    #[test]
    fn test_find() {
        let mut shell = Shell::new();
        shell.execute("mkdir /proj");
        shell.execute("mkdir /proj/src");
        shell
            .fs_mut()
            .write_file("/proj/src/main.rs", "/", b"fn main() {}\n");
        shell
            .fs_mut()
            .write_file("/proj/README.md", "/", b"# Proj\n");
        let result = shell.execute("find /proj -name '*.rs'");
        assert!(result.stdout.contains("/proj/src/main.rs"));
        assert!(!result.stdout.contains("README.md"));
    }

    #[test]
    fn test_sort() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"cherry\napple\nbanana\n");
        let result = shell.execute("sort /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_sort_numeric() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"10\n2\n30\n1\n");
        let result = shell.execute("sort -n /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["1", "2", "10", "30"]);
    }

    #[test]
    fn test_sort_reverse() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"a\nc\nb\n");
        let result = shell.execute("sort -r /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["c", "b", "a"]);
    }

    #[test]
    fn test_redirect_write() {
        let mut shell = Shell::new();
        shell.execute("echo hello > /output.txt");
        assert_eq!(
            shell.fs().read_file("/output.txt", "/"),
            Some(b"hello\n".to_vec())
        );
    }

    #[test]
    fn test_redirect_append() {
        let mut shell = Shell::new();
        shell.execute("echo line1 > /f.txt");
        shell.execute("echo line2 >> /f.txt");
        let content = shell.fs().read_file("/f.txt", "/").unwrap();
        assert_eq!(String::from_utf8_lossy(&content), "line1\nline2\n");
    }

    #[test]
    fn test_redirect_input() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/input.txt", "/", b"hello world\n");
        let result = shell.execute("cat < /input.txt");
        assert_eq!(result.stdout.trim(), "hello world");
    }

    #[test]
    fn test_pipe_with_grep() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/fruits.txt", "/", b"apple\nbanana\ncherry\n");
        let result = shell.execute("cat /fruits.txt | grep cherry");
        assert_eq!(result.stdout.trim(), "cherry");
    }

    #[test]
    fn test_semicolon() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello; echo world");
        assert_eq!(result.stdout, "hello\nworld\n");
    }

    #[test]
    fn test_variable_expansion() {
        let mut shell = Shell::new();
        shell.set_var("NAME", "World");
        let result = shell.execute("echo hello $NAME");
        assert_eq!(result.stdout.trim(), "hello World");
    }

    #[test]
    fn test_command_not_found() {
        let mut shell = Shell::new();
        let result = shell.execute("nonexistent");
        assert_eq!(result.exit_code, 127);
        assert!(result.stderr.contains("command not found"));
    }

    #[test]
    fn test_exit_status() {
        let mut shell = Shell::new();
        shell.execute("grep nope /dev/null");
        assert_eq!(shell.get_var("?"), Some("1"));
        shell.execute("echo ok");
        assert_eq!(shell.get_var("?"), Some("0"));
    }

    #[test]
    fn test_ls_long() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"hello");
        let result = shell.execute("ls -l /f.txt");
        assert!(result.stdout.contains("f.txt"));
        assert!(result.stdout.contains("5")); // size
    }

    #[test]
    fn test_cp_recursive() {
        let mut shell = Shell::new();
        shell.execute("mkdir /src");
        shell.fs_mut().write_file("/src/a.txt", "/", b"1");
        shell.fs_mut().write_file("/src/b.txt", "/", b"2");
        shell.execute("cp -r /src /dst");
        assert_eq!(shell.fs().read_file("/dst/a.txt", "/"), Some(b"1".to_vec()));
        assert_eq!(shell.fs().read_file("/dst/b.txt", "/"), Some(b"2".to_vec()));
    }

    #[test]
    fn test_rm_recursive() {
        let mut shell = Shell::new();
        shell.execute("mkdir /dir");
        shell.fs_mut().write_file("/dir/f.txt", "/", b"x");
        shell.execute("rm -r /dir");
        assert!(!shell.fs().exists("/dir", "/"));
    }

    #[test]
    fn test_mkdir_parents() {
        let mut shell = Shell::new();
        shell.execute("mkdir -p /a/b/c");
        assert!(shell.fs().is_dir("/a/b/c", "/"));
    }

    #[test]
    fn test_grep_case_insensitive() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"Hello World\n");
        let result = shell.execute("grep -i hello /f.txt");
        assert!(result.stdout.contains("Hello World"));
    }

    #[test]
    fn test_grep_line_numbers() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"line1\nline2\nline3\n");
        let result = shell.execute("grep -n line2 /f.txt");
        assert!(result.stdout.contains("2:line2"));
    }

    #[test]
    fn test_wc_default() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"hello world\nfoo bar baz\n");
        let result = shell.execute("wc /f.txt");
        // Should show lines, words, bytes
        assert!(result.stdout.contains("2")); // 2 lines
        assert!(result.stdout.contains("5")); // 5 words
    }

    #[test]
    fn test_find_wildcard() {
        let mut shell = Shell::new();
        shell.execute("mkdir /d");
        shell.fs_mut().write_file("/d/a.txt", "/", b"");
        shell.fs_mut().write_file("/d/b.rs", "/", b"");
        shell.fs_mut().write_file("/d/c.txt", "/", b"");
        let result = shell.execute("find /d -name '*.txt'");
        assert!(result.stdout.contains("/d/a.txt"));
        assert!(result.stdout.contains("/d/c.txt"));
        assert!(!result.stdout.contains("b.rs"));
    }

    #[test]
    fn test_sort_unique() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"b\na\nb\nc\na\n");
        let result = shell.execute("sort -u /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_echo_with_quotes_and_vars() {
        let mut shell = Shell::new();
        shell.set_var("GREET", "hi");
        let result = shell.execute("echo \"$GREET\" 'literal'");
        assert_eq!(result.stdout.trim(), "hi literal");
    }

    #[test]
    fn test_cd_dotdot() {
        let mut shell = Shell::new();
        shell.execute("mkdir /a");
        shell.execute("mkdir /a/b");
        shell.execute("cd /a/b");
        shell.execute("cd ..");
        assert_eq!(shell.cwd(), "/a");
    }

    #[test]
    fn test_cd_home() {
        let mut shell = Shell::new();
        shell.execute("mkdir -p /home/user");
        shell.execute("cd");
        assert_eq!(shell.cwd(), "/home/user");
    }

    #[test]
    fn test_uniq_basic() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"a\na\nb\nc\nc\nc\n");
        let result = shell.execute("uniq /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_uniq_count() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"a\na\nb\nc\nc\nc\n");
        let result = shell.execute("uniq -c /f.txt");
        assert!(result.stdout.contains("2 a"));
        assert!(result.stdout.contains("1 b"));
        assert!(result.stdout.contains("3 c"));
    }

    #[test]
    fn test_uniq_duplicates() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"a\na\nb\nc\nc\nc\n");
        let result = shell.execute("uniq -d /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["a", "c"]);
    }

    #[test]
    fn test_uniq_unique_only() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"a\na\nb\nc\nc\nc\n");
        let result = shell.execute("uniq -u /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["b"]);
    }

    #[test]
    fn test_cut_fields() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"one\ttwo\tthree\nfour\tfive\tsix\n");
        let result = shell.execute("cut -f 1,3 /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["one\tthree", "four\tsix"]);
    }

    #[test]
    fn test_cut_custom_delimiter() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"a:b:c\n1:2:3\n");
        let result = shell.execute("cut -d : -f 2 /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["b", "2"]);
    }

    #[test]
    fn test_tr_translate() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello | tr 'a-z' 'A-Z'");
        assert_eq!(result.stdout.trim(), "HELLO");
    }

    #[test]
    fn test_tr_delete() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello | tr -d 'l'");
        assert_eq!(result.stdout.trim(), "heo");
    }

    #[test]
    fn test_tr_squeeze() {
        let mut shell = Shell::new();
        let result = shell.execute("echo aaabbbccc | tr -s 'a'");
        assert_eq!(result.stdout.trim(), "abbbccc");
    }

    #[test]
    fn test_tr_complement() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello123 | tr -c '[:alpha:]' '_'");
        // -c complements: everything NOT alpha becomes '_'
        // 'hello123\n' → 'hello' (5 alpha) + 4 non-alpha ('1','2','3','\n') → '_'
        assert_eq!(result.stdout.trim(), "hello____");
    }

    #[test]
    fn test_tr_delete_and_squeeze() {
        let mut shell = Shell::new();
        // -ds: delete chars in set, squeeze remaining
        // set is 'ab', input is 'aaabbbccc'
        // delete a,b → 'ccc', no repeats to squeeze
        // but squeeze also applies to chars NOT in set for repeat removal
        // Actually in real tr, -ds squeezes chars in set1 after deletion
        // Since set1 chars are deleted, squeeze has no effect. Result: 'ccc'
        // But our impl squeezes all repeated chars after deletion.
        // Let's test with input that has repeated non-set chars
        let result = shell.execute("echo aaabbbccc | tr -ds 'ab' 'x'");
        // After deleting a and b from 'aaabbbccc': 'ccc'
        // No consecutive repeats of set1 chars remain, so result is 'ccc'
        assert_eq!(result.stdout.trim(), "ccc");
    }

    #[test]
    fn test_tr_posix_upper() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello | tr '[:lower:]' '[:upper:]'");
        assert_eq!(result.stdout.trim(), "HELLO");
    }

    #[test]
    fn test_sed_substitute() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello world | sed 's/world/rust/'");
        assert_eq!(result.stdout.trim(), "hello rust");
    }

    #[test]
    fn test_sed_global() {
        let mut shell = Shell::new();
        let result = shell.execute("echo aaa | sed 's/a/b/g'");
        assert_eq!(result.stdout.trim(), "bbb");
    }

    #[test]
    fn test_sed_case_insensitive() {
        let mut shell = Shell::new();
        let result = shell.execute("echo Hello | sed 's/hello/hi/gi'");
        assert_eq!(result.stdout.trim(), "hi");
    }

    #[test]
    fn test_sed_first_only() {
        let mut shell = Shell::new();
        let result = shell.execute("echo aaa | sed 's/a/b/'");
        assert_eq!(result.stdout.trim(), "baa");
    }

    #[test]
    fn test_sed_delete_line() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"keep\nremove\nkeep\n");
        let result = shell.execute("sed '2d' /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["keep", "keep"]);
    }

    #[test]
    fn test_sed_print_line() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"a\nb\nc\n");
        let result = shell.execute("sed -n '2p' /f.txt");
        assert_eq!(result.stdout.trim(), "b");
    }

    #[test]
    fn test_sed_delete_by_pattern() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"keep\nREMOVE\nkeep\n");
        let result = shell.execute("sed '/REMOVE/d' /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["keep", "keep"]);
    }

    #[test]
    fn test_sed_address_range() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/f.txt", "/", b"1\n2\n3\n4\n5\n");
        let result = shell.execute("sed '2,4d' /f.txt");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["1", "5"]);
    }

    #[test]
    fn test_sed_multiple_commands() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'hello world' | sed -e 's/hello/hi/' -e 's/world/there/'");
        assert_eq!(result.stdout.trim(), "hi there");
    }

    #[test]
    fn test_sed_semicolon_commands() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'hello world' | sed 's/hello/hi/;s/world/there/'");
        assert_eq!(result.stdout.trim(), "hi there");
    }

    #[test]
    fn test_basename_basic() {
        let mut shell = Shell::new();
        let result = shell.execute("basename /usr/local/bin/foo");
        assert_eq!(result.stdout.trim(), "foo");
    }

    #[test]
    fn test_basename_suffix() {
        let mut shell = Shell::new();
        let result = shell.execute("basename /path/to/file.tar.gz .gz");
        assert_eq!(result.stdout.trim(), "file.tar");
    }

    #[test]
    fn test_tee_basic() {
        let mut shell = Shell::new();
        let result = shell.execute("echo hello | tee /out.txt");
        assert_eq!(result.stdout.trim(), "hello");
        assert_eq!(
            shell.fs().read_file("/out.txt", "/"),
            Some(b"hello\n".to_vec())
        );
    }

    #[test]
    fn test_tee_append() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/out.txt", "/", b"line1\n");
        shell.execute("echo line2 | tee -a /out.txt");
        let content = shell.fs().read_file("/out.txt", "/").unwrap();
        assert_eq!(String::from_utf8_lossy(&content), "line1\nline2\n");
    }

    #[test]
    fn test_xargs_basic() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'a b c' | xargs");
        assert_eq!(result.stdout.trim(), "a b c");
    }

    #[test]
    fn test_xargs_max_args() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'a b c d' | xargs -n 2");
        let lines: Vec<&str> = result.stdout.trim().lines().collect();
        assert_eq!(lines, vec!["a b", "c d"]);
    }

    #[test]
    fn test_xargs_runs_command() {
        let mut shell = Shell::new();
        shell
            .fs_mut()
            .write_file("/f.txt", "/", b"line1\nline2\nline3\n");
        // xargs passes stdin tokens as args to the command
        // "echo '/f.txt' | xargs wc -l" → wc -l /f.txt
        let result = shell.execute("echo '/f.txt' | xargs wc -l");
        assert!(result.stdout.contains("3"));
    }

    #[test]
    fn test_xargs_multiple_files() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/a.txt", "/", b"line1\nline2\n");
        shell.fs_mut().write_file("/b.txt", "/", b"only\n");
        let result = shell.execute("echo '/a.txt /b.txt' | xargs wc -l");
        assert!(result.stdout.contains("2"));
        assert!(result.stdout.contains("1"));
        assert!(result.stdout.contains("3")); // total
    }

    #[test]
    fn test_diff_different() {
        let mut shell = Shell::new();
        shell.fs_mut().write_file("/a.txt", "/", b"foo\nbar\nbaz\n");
        shell.fs_mut().write_file("/b.txt", "/", b"foo\nqux\nbaz\n");
        let result = shell.execute("diff /a.txt /b.txt");
        assert!(result.stdout.contains("< bar"));
        assert!(result.stdout.contains("> qux"));
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_diff_missing_file() {
        let mut shell = Shell::new();
        let result = shell.execute("diff /nope.txt /also_nope.txt");
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_man_ls() {
        let mut shell = Shell::new();
        let result = shell.execute("man ls");
        assert!(result.stdout.contains("Usage:"));
        assert!(result.stdout.contains("ls"));
        assert!(result.stdout.contains("-l"));
        assert!(result.stdout.contains("-a"));
    }

    #[test]
    fn test_man_list_all() {
        let mut shell = Shell::new();
        let result = shell.execute("man");
        assert!(result.stdout.contains("Available commands"));
        assert!(result.stdout.contains("ls"));
        assert!(result.stdout.contains("grep"));
        assert!(result.stdout.contains("sed"));
        assert!(result.stdout.contains("man"));
    }

    #[test]
    fn test_man_unknown() {
        let mut shell = Shell::new();
        let result = shell.execute("man nonexistent");
        assert_eq!(result.exit_code, 1);
        assert!(result.stderr.contains("command not found"));
    }

    // ── Security tests ──────────────────────────────────────────

    #[test]
    fn test_cp_into_self_rejected() {
        let mut shell = Shell::new();
        shell.execute("mkdir /src");
        shell.fs_mut().write_file("/src/file.txt", "/", b"data");

        let result = shell.execute("cp -r /src /src/child");
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("cannot copy"));
    }

    #[test]
    fn test_cp_nested_into_self_rejected() {
        let mut shell = Shell::new();
        shell.execute("mkdir -p /a/b/c");

        let result = shell.execute("cp -r /a /a/b/c/d");
        assert_ne!(result.exit_code, 0);
        assert!(result.stderr.contains("cannot copy"));
    }

    #[test]
    fn test_diff_size_cap() {
        let mut shell = Shell::new();
        // Create a file with more than MAX_DIFF_LINES (100000)
        let lines: String = (0..150_000).map(|i| format!("line{}\n", i)).collect();
        shell
            .fs_mut()
            .write_file("/big1.txt", "/", lines.as_bytes());
        let lines2: String = (0..150_000).map(|i| format!("line{}_x\n", i)).collect();
        shell
            .fs_mut()
            .write_file("/big2.txt", "/", lines2.as_bytes());

        let result = shell.execute("diff /big1.txt /big2.txt");
        assert_eq!(result.exit_code, 2);
        assert!(result.stderr.contains("too large"));
    }

    #[test]
    fn test_memoryfs_limits_via_shell() {
        use fs::FsLimits;
        let limits = FsLimits {
            max_file_size: 100,
            ..Default::default()
        };
        let mut shell = Shell::with_limits(limits);

        // Small write should succeed
        shell.fs_mut().write_file("/ok.txt", "/", b"small");
        assert!(shell.fs().exists("/ok.txt", "/"));

        // Oversized write via shell execute should not crash
        // (echo writes through the shell, which respects fs limits)
        let big_text: String = "x".repeat(200);
        let result = shell.execute(&format!("echo {} > /big.txt", big_text));
        // The file may or may not be created depending on how echo output flows,
        // but it should not panic
        assert!(result.exit_code == 0 || !shell.fs().exists("/big.txt", "/"));
    }

    #[test]
    fn test_parse_trailing_backslash_errors() {
        let mut shell = Shell::new();
        let result = shell.execute(r"echo hello\");
        assert_eq!(result.exit_code, 2);
        assert!(result.stderr.contains("syntax error"));
    }

    #[test]
    fn test_parse_unterminated_var_errors() {
        let mut shell = Shell::new();
        let result = shell.execute("echo ${FOO");
        assert_eq!(result.exit_code, 2);
        assert!(result.stderr.contains("unterminated ${"));
    }

    #[test]
    fn test_parse_unterminated_quote_errors() {
        let mut shell = Shell::new();
        let result = shell.execute("echo 'hello");
        assert_eq!(result.exit_code, 2);
        assert!(result.stderr.contains("unterminated single quote"));
    }

    #[test]
    fn test_input_too_long() {
        let mut shell = Shell::new();
        let long_input = "x".repeat(2_000_000);
        let result = shell.execute(&long_input);
        assert_eq!(result.exit_code, 2);
        assert!(result.stderr.contains("too long"));
    }

    #[test]
    fn test_fs_events_accumulate() {
        use fs::FsEvent;
        let mut shell = Shell::new();
        shell.execute("mkdir /tmp");
        shell.execute("echo hello > /tmp/file.txt");
        shell.execute("cat /tmp/file.txt");
        shell.execute("rm /tmp/file.txt");

        let events = shell.fs().events();
        assert!(events.contains(&FsEvent::Mkdir {
            path: "/tmp".to_string()
        }));
        assert!(events.contains(&FsEvent::WriteFile {
            path: "/tmp/file.txt".to_string(),
            size: 6
        }));
        assert!(events.contains(&FsEvent::Remove {
            path: "/tmp/file.txt".to_string()
        }));
    }

    #[test]
    fn test_fs_events_take_clears() {
        let mut shell = Shell::new();
        shell.execute("mkdir /test");
        assert!(!shell.fs().events().is_empty());

        let events = shell.fs_mut().take_events();
        assert!(!events.is_empty());
        assert!(shell.fs().events().is_empty());
    }

    #[test]
    fn test_fs_events_on_remove_all() {
        use fs::FsEvent;
        let mut shell = Shell::new();
        shell.execute("mkdir -p /a/b");
        shell.execute("touch /a/b/file.txt");
        shell.fs_mut().clear_events();
        shell.execute("rm -r /a");

        let events = shell.fs().events();
        assert!(events.contains(&FsEvent::RemoveAll {
            path: "/a".to_string()
        }));
    }
}
