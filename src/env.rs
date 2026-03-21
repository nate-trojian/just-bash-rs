use std::collections::HashMap;

/// In-memory environment: variables and current working directory.
pub struct Env {
    vars: HashMap<String, String>,
    cwd: String,
}

impl Env {
    /// Create a new environment with sensible defaults.
    pub fn new() -> Self {
        let mut vars = HashMap::new();
        vars.insert("HOME".to_string(), "/home/user".to_string());
        vars.insert("USER".to_string(), "user".to_string());
        vars.insert("SHELL".to_string(), "/bin/just-bash".to_string());
        vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        vars.insert("PWD".to_string(), "/".to_string());
        vars.insert("?".to_string(), "0".to_string());
        Env {
            vars,
            cwd: "/".to_string(),
        }
    }

    /// Get an environment variable.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    /// Set an environment variable.
    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(key.to_string(), value.to_string());
    }

    /// Remove an environment variable.
    pub fn remove(&mut self, key: &str) {
        self.vars.remove(key);
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Set the current working directory.
    pub fn set_cwd(&mut self, path: &str) {
        self.cwd = path.to_string();
        self.vars.insert("PWD".to_string(), path.to_string());
    }

    /// Expand `$VAR` and `${VAR}` in a string.
    /// Also handles `$?` (last exit status), `$$` (pid), `$!`.
    pub fn expand(&self, input: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = input.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' {
                i += 1;
                if i >= chars.len() {
                    result.push('$');
                    break;
                }

                if chars[i] == '{' {
                    // ${VAR}
                    i += 1;
                    let mut var_name = String::new();
                    while i < chars.len() && chars[i] != '}' {
                        var_name.push(chars[i]);
                        i += 1;
                    }
                    if i < chars.len() {
                        i += 1; // skip }
                    }
                    if let Some(val) = self.vars.get(&var_name) {
                        result.push_str(val);
                    }
                } else if chars[i] == '?' || chars[i] == '!' {
                    // Special single-char variables
                    let key = chars[i].to_string();
                    i += 1;
                    if let Some(val) = self.vars.get(&key) {
                        result.push_str(val);
                    }
                } else if chars[i] == '$' {
                    // $$ = process ID (stub: just output "$")
                    i += 1;
                    result.push('$');
                } else if chars[i].is_alphanumeric() || chars[i] == '_' {
                    // $VAR
                    let mut var_name = String::new();
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                        var_name.push(chars[i]);
                        i += 1;
                    }
                    if let Some(val) = self.vars.get(&var_name) {
                        result.push_str(val);
                    }
                } else {
                    result.push('$');
                }
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }

        result
    }
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand() {
        let mut env = Env::new();
        env.set("FOO", "bar");
        assert_eq!(env.expand("$FOO"), "bar");
        assert_eq!(env.expand("${FOO}"), "bar");
        assert_eq!(env.expand("hello $FOO"), "hello bar");
        assert_eq!(env.expand("$MISSING"), "");
        assert_eq!(env.expand("$$"), "$");
    }

    #[test]
    fn test_cwd() {
        let mut env = Env::new();
        assert_eq!(env.cwd(), "/");
        env.set_cwd("/home/user");
        assert_eq!(env.cwd(), "/home/user");
        assert_eq!(env.get("PWD"), Some("/home/user"));
    }
}
