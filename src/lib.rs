pub mod commands;
pub mod env;
pub mod fs;
pub mod parser;

use commands::get_commands;
use env::Env;
use fs::{Fs, FsMode};
use parser::Pipeline;

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

        let pipelines = parser::parse(input, &self.env);

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

        // Read input from file if `<` redirect
        let mut current_stdin = if let Some(ref input_file) = pipeline.input_redirect {
            match self.fs.read_file(input_file, self.env.cwd()) {
                Some(content) => String::from_utf8_lossy(&content).to_string(),
                None => {
                    return ExecuteResult {
                        stdout: String::new(),
                        stderr: format!("{}: No such file or directory\n", input_file),
                        exit_code: 1,
                    };
                }
            }
        } else {
            String::new()
        };

        let mut final_stdout = String::new();
        let mut final_stderr = String::new();
        let mut exit_code = 0;

        for (i, cmd) in pipeline.commands.iter().enumerate() {
            if cmd.args.is_empty() {
                continue;
            }

            let name = &cmd.args[0];
            let args = &cmd.args[1..];

            if let Some(handler) = commands.get(name.as_str()) {
                let (stdout, stderr, code) =
                    handler(args, &current_stdin, &mut self.fs, &mut self.env);
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

        // Apply output redirect
        if let Some(ref output_file) = pipeline.output_redirect {
            if pipeline.append {
                if let Some(existing) = self.fs.read_file(output_file, self.env.cwd()) {
                    let mut combined = String::from_utf8_lossy(&existing).to_string();
                    combined.push_str(&final_stdout);
                    self.fs
                        .write_file(output_file, self.env.cwd(), combined.as_bytes());
                } else {
                    self.fs
                        .write_file(output_file, self.env.cwd(), final_stdout.as_bytes());
                }
            } else {
                self.fs
                    .write_file(output_file, self.env.cwd(), final_stdout.as_bytes());
            }
            final_stdout = String::new();
        }

        ExecuteResult {
            stdout: final_stdout,
            stderr: final_stderr,
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
}
