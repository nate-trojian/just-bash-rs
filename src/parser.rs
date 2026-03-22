use crate::env::Env;

// ── Error type ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    fn new(msg: &str) -> Self {
        ParseError {
            message: msg.to_string(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "syntax error: {}", self.message)
    }
}

// ── AST types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleCommand {
    pub args: Vec<String>,
    pub input_redirect: Option<String>,
    pub output_redirect: Option<String>,
    pub append: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pipeline {
    pub commands: Vec<SimpleCommand>,
    pub input_redirect: Option<String>,
    pub output_redirect: Option<String>,
    pub append: bool,
}

// ── Public API ────────────────────────────────────────────────────

/// Parse a full input line (possibly containing `;`-separated statements)
/// into a list of pipelines, with variables expanded.
pub fn parse(input: &str, env: &Env) -> Result<Vec<Pipeline>, ParseError> {
    let statements = split_statements(input);
    let mut pipelines = Vec::new();
    for s in statements {
        if s.trim().is_empty() {
            continue;
        }
        pipelines.push(parse_statement(&s, env)?);
    }
    Ok(pipelines)
}

// ── Statement splitting ───────────────────────────────────────────

fn split_statements(input: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            '\\' if !in_single => {
                current.push(ch);
                if let Some(&_next) = chars.peek() {
                    current.push(chars.next().unwrap());
                }
            }
            ';' if !in_single && !in_double => {
                statements.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        statements.push(current);
    }
    statements
}

// ── Statement → Pipeline ──────────────────────────────────────────

fn parse_statement(input: &str, env: &Env) -> Result<Pipeline, ParseError> {
    let parts = split_pipes(input);
    let mut commands = Vec::new();

    for part in &parts {
        let expanded = expand_variables(part, env)?;
        let tokens = tokenize(&expanded)?;
        let cmd = parse_simple_command(tokens);
        commands.push(cmd);
    }

    let mut pipeline = Pipeline {
        commands,
        input_redirect: None,
        output_redirect: None,
        append: false,
    };

    // Hoist redirects from first / last command to pipeline level
    if let Some(first) = pipeline.commands.first_mut() {
        if first.input_redirect.is_some() {
            pipeline.input_redirect = first.input_redirect.take();
        }
    }
    if let Some(last) = pipeline.commands.last_mut() {
        if last.output_redirect.is_some() {
            pipeline.output_redirect = last.output_redirect.take();
            pipeline.append = last.append;
        }
    }

    Ok(pipeline)
}

fn split_pipes(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(chars[i]);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(chars[i]);
            }
            '\\' if !in_single => {
                current.push(chars[i]);
                i += 1;
                if i < chars.len() {
                    current.push(chars[i]);
                }
            }
            '|' if !in_single && !in_double => {
                parts.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(chars[i]),
        }
        i += 1;
    }
    if !current.is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

// ── Tokenizer ─────────────────────────────────────────────────────

fn tokenize(input: &str) -> Result<Vec<String>, ParseError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            i += 1;
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    i += 1;
                    if i < chars.len() {
                        current.push(chars[i]);
                    } else {
                        return Err(ParseError::new("unexpected EOF after '\\'"));
                    }
                }
                _ => current.push(ch),
            }
            i += 1;
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            '>' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                i += 1;
                if i < chars.len() && chars[i] == '>' {
                    tokens.push(">>".to_string());
                } else {
                    tokens.push(">".to_string());
                    continue; // don't increment again
                }
            }
            '<' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                tokens.push("<".to_string());
            }
            '\\' => {
                i += 1;
                if i < chars.len() {
                    current.push(chars[i]);
                } else {
                    return Err(ParseError::new("unexpected EOF after '\\'"));
                }
            }
            _ => current.push(ch),
        }
        i += 1;
    }

    if in_single {
        return Err(ParseError::new("unterminated single quote"));
    }
    if in_double {
        return Err(ParseError::new("unterminated double quote"));
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

// ── SimpleCommand parser ──────────────────────────────────────────

fn parse_simple_command(tokens: Vec<String>) -> SimpleCommand {
    let mut args = Vec::new();
    let mut input_redirect = None;
    let mut output_redirect = None;
    let mut append = false;
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].as_str() {
            ">" => {
                i += 1;
                if i < tokens.len() {
                    output_redirect = Some(tokens[i].clone());
                }
            }
            ">>" => {
                i += 1;
                if i < tokens.len() {
                    output_redirect = Some(tokens[i].clone());
                    append = true;
                }
            }
            "<" => {
                i += 1;
                if i < tokens.len() {
                    input_redirect = Some(tokens[i].clone());
                }
            }
            _ => args.push(tokens[i].clone()),
        }
        i += 1;
    }

    SimpleCommand {
        args,
        input_redirect,
        output_redirect,
        append,
    }
}

// ── Variable expansion ────────────────────────────────────────────

fn expand_variables(input: &str, env: &Env) -> Result<String, ParseError> {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;

    while i < chars.len() {
        let ch = chars[i];

        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                result.push(ch);
            }
            i += 1;
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    result.push(ch);
                    i += 1;
                    if i < chars.len() {
                        result.push(chars[i]);
                    } else {
                        return Err(ParseError::new("unexpected EOF after '\\'"));
                    }
                }
                '$' => {
                    i += 1;
                    i = expand_var_at(&chars, i, env, &mut result)?;
                    continue;
                }
                _ => result.push(ch),
            }
            i += 1;
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                result.push(ch);
                i += 1;
                if i < chars.len() {
                    result.push(chars[i]);
                } else {
                    return Err(ParseError::new("unexpected EOF after '\\'"));
                }
            }
            '$' => {
                i += 1;
                i = expand_var_at(&chars, i, env, &mut result)?;
                continue;
            }
            _ => result.push(ch),
        }
        i += 1;
    }

    if in_single {
        return Err(ParseError::new("unterminated single quote"));
    }
    if in_double {
        return Err(ParseError::new("unterminated double quote"));
    }

    Ok(result)
}

fn expand_var_at(
    chars: &[char],
    mut i: usize,
    env: &Env,
    result: &mut String,
) -> Result<usize, ParseError> {
    if i >= chars.len() {
        result.push('$');
        return Ok(i);
    }

    if chars[i] == '{' {
        // ${VAR}
        i += 1;
        let mut var_name = String::new();
        while i < chars.len() && chars[i] != '}' {
            var_name.push(chars[i]);
            i += 1;
        }
        if i >= chars.len() {
            return Err(ParseError::new("unterminated ${"));
        }
        i += 1; // skip }
        if let Some(val) = env.get(&var_name) {
            result.push_str(val);
        }
    } else if chars[i].is_alphanumeric()
        || chars[i] == '_'
        || chars[i] == '?'
        || chars[i] == '$'
        || chars[i] == '!'
    {
        let mut var_name = String::new();
        while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '?')
        {
            var_name.push(chars[i]);
            i += 1;
        }
        if let Some(val) = env.get(&var_name) {
            result.push_str(val);
        }
    } else {
        result.push('$');
    }
    Ok(i)
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(input: &str) -> Vec<String> {
        tokenize(input).unwrap()
    }

    fn expand(input: &str, env: &Env) -> String {
        expand_variables(input, env).unwrap()
    }

    #[test]
    fn test_tokenize_simple() {
        assert_eq!(tok("hello world"), vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_quotes() {
        assert_eq!(tok("'hello world'"), vec!["hello world"]);
        assert_eq!(tok("\"hello world\""), vec!["hello world"]);
        assert_eq!(tok("echo 'hello world'"), vec!["echo", "hello world"]);
    }

    #[test]
    fn test_tokenize_escape() {
        assert_eq!(tok(r"hello\ world"), vec!["hello world"]);
    }

    #[test]
    fn test_tokenize_redirect() {
        assert_eq!(
            tok("echo hello > file.txt"),
            vec!["echo", "hello", ">", "file.txt"]
        );
        assert_eq!(
            tok("echo hello >> file.txt"),
            vec!["echo", "hello", ">>", "file.txt"]
        );
        assert_eq!(tok("cat < file.txt"), vec!["cat", "<", "file.txt"]);
    }

    #[test]
    fn test_expand_variables() {
        let mut env = Env::new();
        env.set("FOO", "bar");
        assert_eq!(expand("$FOO", &env), "bar");
        assert_eq!(expand("${FOO}", &env), "bar");
        assert_eq!(expand("hello $FOO", &env), "hello bar");
        assert_eq!(expand("'$FOO'", &env), "$FOO"); // single quotes
        assert_eq!(expand("\"$FOO\"", &env), "bar"); // double quotes
    }

    #[test]
    fn test_parse_simple() {
        let env = Env::new();
        let pipelines = parse("echo hello", &env).unwrap();
        assert_eq!(pipelines.len(), 1);
        assert_eq!(pipelines[0].commands[0].args, vec!["echo", "hello"]);
    }

    #[test]
    fn test_parse_pipe() {
        let env = Env::new();
        let pipelines = parse("echo hello | grep h", &env).unwrap();
        assert_eq!(pipelines.len(), 1);
        assert_eq!(pipelines[0].commands.len(), 2);
        assert_eq!(pipelines[0].commands[0].args, vec!["echo", "hello"]);
        assert_eq!(pipelines[0].commands[1].args, vec!["grep", "h"]);
    }

    #[test]
    fn test_parse_redirect() {
        let env = Env::new();
        let pipelines = parse("echo hello > output.txt", &env).unwrap();
        assert_eq!(pipelines[0].output_redirect, Some("output.txt".to_string()));
        assert!(!pipelines[0].append);
    }

    #[test]
    fn test_parse_append() {
        let env = Env::new();
        let pipelines = parse("echo hello >> output.txt", &env).unwrap();
        assert_eq!(pipelines[0].output_redirect, Some("output.txt".to_string()));
        assert!(pipelines[0].append);
    }

    #[test]
    fn test_parse_input_redirect() {
        let env = Env::new();
        let pipelines = parse("cat < input.txt", &env).unwrap();
        assert_eq!(pipelines[0].input_redirect, Some("input.txt".to_string()));
    }

    #[test]
    fn test_parse_semicolon() {
        let env = Env::new();
        let pipelines = parse("echo hello; echo world", &env).unwrap();
        assert_eq!(pipelines.len(), 2);
        assert_eq!(pipelines[0].commands[0].args, vec!["echo", "hello"]);
        assert_eq!(pipelines[1].commands[0].args, vec!["echo", "world"]);
    }

    #[test]
    fn test_parse_complex() {
        let env = Env::new();
        let pipelines = parse("echo hello | grep h > /tmp/out; cat < /tmp/out", &env).unwrap();
        assert_eq!(pipelines.len(), 2);
        assert_eq!(pipelines[0].commands.len(), 2);
        assert_eq!(pipelines[0].output_redirect, Some("/tmp/out".to_string()));
        assert_eq!(pipelines[1].input_redirect, Some("/tmp/out".to_string()));
    }

    #[test]
    fn test_expand_exit_status() {
        let mut env = Env::new();
        env.set("?", "1");
        assert_eq!(expand("$?", &env), "1");
    }

    #[test]
    fn test_tokenize_adjacent_redirect() {
        assert_eq!(tok("echo hello>file"), vec!["echo", "hello", ">", "file"]);
    }

    // ── Error cases ─────────────────────────────────────────────

    #[test]
    fn test_trailing_backslash_tokenize() {
        let result = tokenize(r"hello\");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("EOF after"));
    }

    #[test]
    fn test_trailing_backslash_expand() {
        let env = Env::new();
        let result = expand_variables(r"hello\", &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("EOF after"));
    }

    #[test]
    fn test_unterminated_dollar_brace() {
        let env = Env::new();
        let result = expand_variables("${FOO", &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unterminated ${"));
    }

    #[test]
    fn test_unterminated_single_quote() {
        let result = tokenize("'hello");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("unterminated single quote"));
    }

    #[test]
    fn test_unterminated_double_quote() {
        let result = tokenize("\"hello");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("unterminated double quote"));
    }

    #[test]
    fn test_parse_trailing_backslash() {
        let env = Env::new();
        let result = parse(r"echo hello\", &env);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unterminated_var() {
        let env = Env::new();
        let result = parse("echo ${FOO", &env);
        assert!(result.is_err());
    }
}
