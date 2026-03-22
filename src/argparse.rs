use std::collections::HashMap;

// ══════════════════════════════════════════════════════════════════
// Metadata types
// ══════════════════════════════════════════════════════════════════

/// How a command uses stdin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StdinBehavior {
    Never,
    Optional,
    Required,
}

/// Definition of a command-line flag.
#[derive(Debug, Clone, Copy)]
pub struct FlagMeta {
    pub short: char,
    pub long: Option<&'static str>,
    pub takes_value: bool,
    pub value_hint: &'static str,
    pub description: &'static str,
}

/// Definition of a positional argument.
#[derive(Debug, Clone, Copy)]
pub struct PositionalMeta {
    pub name: &'static str,
    pub required: bool,
    pub variadic: bool,
    pub description: &'static str,
}

/// Full metadata for a command.
#[derive(Debug, Clone, Copy)]
pub struct CommandMeta {
    pub name: &'static str,
    pub synopsis: &'static str,
    pub description: &'static str,
    pub details: &'static str,
    pub flags: &'static [FlagMeta],
    pub positional: &'static [PositionalMeta],
    pub stdin: StdinBehavior,
}

// ══════════════════════════════════════════════════════════════════
// Parsed result
// ══════════════════════════════════════════════════════════════════

/// Value of a parsed flag.
#[derive(Debug, Clone, PartialEq)]
pub enum FlagValue {
    Bool(bool),
    Value(String),
}

impl FlagValue {
    pub fn is_set(&self) -> bool {
        matches!(self, FlagValue::Bool(true) | FlagValue::Value(_))
    }

    pub fn value(&self) -> Option<&str> {
        match self {
            FlagValue::Value(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

/// Result of parsing command arguments.
#[derive(Debug)]
pub struct ParsedArgs {
    flags: HashMap<char, FlagValue>,
    pub positional: Vec<String>,
    pub errors: Vec<String>,
}

impl ParsedArgs {
    /// Check if a boolean flag is set.
    pub fn has_flag(&self, short: char) -> bool {
        self.flags.get(&short).map_or(false, |v| v.is_set())
    }

    /// Get the value of a value-bearing flag.
    pub fn flag_value(&self, short: char) -> Option<&str> {
        self.flags.get(&short).and_then(|v| v.value())
    }

    /// Get a boolean flag value (true if set, false otherwise).
    pub fn flag_bool(&self, short: char) -> bool {
        self.has_flag(short)
    }
}

// ══════════════════════════════════════════════════════════════════
// Parser
// ══════════════════════════════════════════════════════════════════

/// Parse arguments according to command metadata.
///
/// Handles:
/// - Combined boolean flags: `-la` → sets `l` and `a`
/// - Value flags: `-n 10` → `n` = Value("10")
/// - Numeric shorthand: `-10` → `n` = Value("10") (if command has `-n` with value)
/// - Long flags: `-name PATTERN` (single-dash long form, as in find)
/// - `--` stops flag parsing, rest are positional
pub fn parse_args(meta: &CommandMeta, args: &[String]) -> ParsedArgs {
    let mut result = ParsedArgs {
        flags: HashMap::new(),
        positional: Vec::new(),
        errors: Vec::new(),
    };

    // Pre-populate all boolean flags as false
    for fmeta in meta.flags {
        if !fmeta.takes_value {
            result.flags.insert(fmeta.short, FlagValue::Bool(false));
        }
    }

    // Build lookup maps
    let long_to_short: HashMap<&str, char> = meta
        .flags
        .iter()
        .filter_map(|f| f.long.map(|l| (l, f.short)))
        .collect();

    let flag_meta: HashMap<char, &FlagMeta> = meta.flags.iter().map(|f| (f.short, f)).collect();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // `--` stops flag parsing
        if arg == "--" {
            i += 1;
            while i < args.len() {
                result.positional.push(args[i].clone());
                i += 1;
            }
            break;
        }

        // Long flag with single dash: -name PATTERN
        if arg.starts_with('-') && arg.len() > 2 && !arg[1..].starts_with('-') {
            let name = &arg[1..];
            if let Some(&short) = long_to_short.get(name) {
                if let Some(fmeta) = flag_meta.get(&short) {
                    if fmeta.takes_value {
                        if i + 1 < args.len() {
                            result
                                .flags
                                .insert(short, FlagValue::Value(args[i + 1].clone()));
                            i += 2;
                            continue;
                        } else {
                            result
                                .errors
                                .push(format!("{}: option requires a value: {}", meta.name, arg));
                            i += 1;
                            continue;
                        }
                    } else {
                        result.flags.insert(short, FlagValue::Bool(true));
                        i += 1;
                        continue;
                    }
                }
            }
        }

        // Short flags
        if arg.starts_with('-') && arg.len() > 1 && !arg[1..].starts_with('-') {
            let chars: Vec<char> = arg[1..].chars().collect();

            // Check if this is a numeric shorthand (e.g., -10 for head/tail -n 10)
            if chars.len() > 1 && chars.iter().all(|c| c.is_ascii_digit()) {
                // Find if there's a value-bearing flag that could consume this
                // Look for a flag that takes a value and isn't set yet
                let numeric_str: String = chars.iter().collect();
                let mut used_shorthand = false;
                for fmeta in meta.flags {
                    if fmeta.takes_value && !result.flags.contains_key(&fmeta.short) {
                        result
                            .flags
                            .insert(fmeta.short, FlagValue::Value(numeric_str.clone()));
                        used_shorthand = true;
                        break;
                    }
                }
                if used_shorthand {
                    i += 1;
                    continue;
                }
            }

            // Normal combined flag parsing
            let mut j = 0;
            while j < chars.len() {
                let ch = chars[j];
                if let Some(fmeta) = flag_meta.get(&ch) {
                    if fmeta.takes_value {
                        // Value-bearing flag
                        let rest: String = chars[j + 1..].iter().collect();
                        if !rest.is_empty() {
                            // -n10 (value attached)
                            result.flags.insert(ch, FlagValue::Value(rest));
                        } else if i + 1 < args.len() {
                            // -n 10 (value in next arg)
                            result
                                .flags
                                .insert(ch, FlagValue::Value(args[i + 1].clone()));
                            i += 1;
                        } else {
                            result
                                .errors
                                .push(format!("{}: option requires a value: -{}", meta.name, ch));
                        }
                        j = chars.len(); // done with this arg
                    } else {
                        // Boolean flag
                        result.flags.insert(ch, FlagValue::Bool(true));
                        j += 1;
                    }
                } else {
                    result
                        .errors
                        .push(format!("{}: invalid option: -{}", meta.name, ch));
                    j += 1;
                }
            }
            i += 1;
            continue;
        }

        // Positional argument
        result.positional.push(arg.clone());
        i += 1;
    }

    result
}

// ══════════════════════════════════════════════════════════════════
// Help formatter
// ══════════════════════════════════════════════════════════════════

/// Format help text for a command in a compact help-style format.
pub fn format_help(meta: &CommandMeta) -> String {
    let mut out = String::new();

    // Usage line
    out.push_str(&format!("Usage: {}\n", meta.synopsis));

    // Description
    if !meta.description.is_empty() {
        out.push('\n');
        out.push_str(meta.description);
        out.push('\n');
    }

    // Details
    if !meta.details.is_empty() {
        out.push('\n');
        for line in meta.details.lines() {
            out.push_str(line);
            out.push('\n');
        }
    }

    // Options
    if !meta.flags.is_empty() {
        out.push_str("\nOptions:\n");
        for fmeta in meta.flags {
            let flag_str = if let Some(long) = fmeta.long {
                if fmeta.takes_value {
                    format!("-{}, -{} {}", fmeta.short, long, fmeta.value_hint)
                } else {
                    format!("-{}, -{}", fmeta.short, long)
                }
            } else if fmeta.takes_value {
                format!("-{} {}", fmeta.short, fmeta.value_hint)
            } else {
                format!("-{}", fmeta.short)
            };
            out.push_str(&format!("  {:<20} {}\n", flag_str, fmeta.description));
        }
    }

    // Positional args
    if !meta.positional.is_empty() {
        out.push_str("\nArguments:\n");
        for pmeta in meta.positional {
            out.push_str(&format!("  {:<20} {}\n", pmeta.name, pmeta.description));
        }
    }

    // Stdin
    if meta.stdin == StdinBehavior::Optional || meta.stdin == StdinBehavior::Required {
        out.push_str("\nReads from stdin if no file arguments given.\n");
    }

    out
}

/// Format a list of all commands with their descriptions.
pub fn format_command_list(commands: &[(&str, &CommandMeta)]) -> String {
    let mut out = String::new();
    out.push_str("Available commands:\n\n");

    let mut sorted: Vec<(&str, &CommandMeta)> = commands.to_vec();
    sorted.sort_by_key(|(name, _)| *name);

    for (name, meta) in &sorted {
        out.push_str(&format!("  {:<12} {}\n", name, meta.description));
    }

    out.push_str("\nUse 'man COMMAND' for details.\n");
    out
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta() -> CommandMeta {
        CommandMeta {
            name: "test",
            synopsis: "test [-la] [-n NUM] [path...]",
            description: "A test command",
            details: "",
            flags: &[
                FlagMeta {
                    short: 'l',
                    long: None,
                    takes_value: false,
                    value_hint: "",
                    description: "Long format",
                },
                FlagMeta {
                    short: 'a',
                    long: Some("all"),
                    takes_value: false,
                    value_hint: "",
                    description: "Show all",
                },
                FlagMeta {
                    short: 'n',
                    long: Some("number"),
                    takes_value: true,
                    value_hint: "NUM",
                    description: "Set number",
                },
            ],
            positional: &[PositionalMeta {
                name: "path",
                required: false,
                variadic: true,
                description: "Paths to process",
            }],
            stdin: StdinBehavior::Never,
        }
    }

    fn s(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn test_boolean_flags() {
        let meta = test_meta();
        let args = vec![s("-l"), s("-a"), s("/foo")];
        let parsed = parse_args(&meta, &args);
        assert!(parsed.has_flag('l'));
        assert!(parsed.has_flag('a'));
        assert_eq!(parsed.positional, vec!["/foo"]);
        assert!(parsed.errors.is_empty());
    }

    #[test]
    fn test_combined_flags() {
        let meta = test_meta();
        let args = vec![s("-la"), s("/foo")];
        let parsed = parse_args(&meta, &args);
        assert!(parsed.has_flag('l'));
        assert!(parsed.has_flag('a'));
        assert_eq!(parsed.positional, vec!["/foo"]);
    }

    #[test]
    fn test_value_flag_attached() {
        let meta = test_meta();
        let args = vec![s("-n42")];
        let parsed = parse_args(&meta, &args);
        assert_eq!(parsed.flag_value('n'), Some("42"));
    }

    #[test]
    fn test_value_flag_separate() {
        let meta = test_meta();
        let args = vec![s("-n"), s("42")];
        let parsed = parse_args(&meta, &args);
        assert_eq!(parsed.flag_value('n'), Some("42"));
    }

    #[test]
    fn test_long_flag() {
        let meta = test_meta();
        let args = vec![s("-all")];
        let parsed = parse_args(&meta, &args);
        assert!(parsed.has_flag('a'));
    }

    #[test]
    fn test_long_value_flag() {
        let meta = test_meta();
        let args = vec![s("-number"), s("42")];
        let parsed = parse_args(&meta, &args);
        assert_eq!(parsed.flag_value('n'), Some("42"));
    }

    #[test]
    fn test_numeric_shorthand() {
        let meta = test_meta();
        let args = vec![s("-10")];
        let parsed = parse_args(&meta, &args);
        assert_eq!(parsed.flag_value('n'), Some("10"));
    }

    #[test]
    fn test_unknown_flag() {
        let meta = test_meta();
        let args = vec![s("-x")];
        let parsed = parse_args(&meta, &args);
        assert!(!parsed.errors.is_empty());
        assert!(parsed.errors[0].contains("invalid option"));
    }

    #[test]
    fn test_stop_parsing() {
        let meta = test_meta();
        let args = vec![s("-l"), s("--"), s("-a"), s("file")];
        let parsed = parse_args(&meta, &args);
        assert!(parsed.has_flag('l'));
        assert!(!parsed.has_flag('a'));
        assert_eq!(parsed.positional, vec!["-a", "file"]);
    }

    #[test]
    fn test_missing_value() {
        let meta = test_meta();
        let args = vec![s("-n")];
        let parsed = parse_args(&meta, &args);
        assert!(!parsed.errors.is_empty());
        assert!(parsed.errors[0].contains("requires a value"));
    }

    #[test]
    fn test_format_help() {
        let meta = test_meta();
        let help = format_help(&meta);
        assert!(help.contains("Usage: test"));
        assert!(help.contains("-l"));
        assert!(help.contains("-a"));
        assert!(help.contains("-n NUM"));
        assert!(help.contains("Long format"));
    }
}
