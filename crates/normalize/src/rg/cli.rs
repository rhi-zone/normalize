// rg subcommand — CLI argument parsing for ripgrep drop-in.
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Cli {
    // Pattern
    pub patterns: Vec<String>,
    pub fixed_strings: bool,
    pub word_regexp: bool,
    pub ignore_case: bool,
    pub smart_case: bool,
    pub invert_match: bool,
    pub multiline: bool,

    // Output
    pub line_number: Option<bool>,
    pub no_heading: Option<bool>,
    pub with_filename: Option<bool>,
    pub only_matching: bool,
    pub files_with_matches: bool,
    pub files_without_match: bool,
    pub count: bool,
    pub null: bool,
    pub quiet: bool,
    pub stats: bool,
    pub files: bool,

    // Context
    pub before_context: usize,
    pub after_context: usize,

    // Limits
    pub max_count: Option<u64>,
    pub max_depth: Option<usize>,

    // Path filtering
    pub globs: Vec<String>,
    pub hidden: bool,
    pub no_ignore: bool,
    pub no_ignore_vcs: bool,
    pub follow: bool,

    // Display
    pub color: ColorChoice,

    // Positional
    pub pattern: Option<String>,
    pub paths: Vec<PathBuf>,

    // Meta
    pub version: bool,
    pub help: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl Cli {
    pub fn parse(iter: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut cli = Self::default();
        let args: Vec<OsString> = iter.collect();
        let mut i = 0;
        let mut first_positional = true;

        while i < args.len() {
            let arg = args[i].to_str().unwrap_or("");

            if arg == "--" {
                i += 1;
                while i < args.len() {
                    cli.paths.push(PathBuf::from(&args[i]));
                    i += 1;
                }
                break;
            }

            if let Some(rest) = arg.strip_prefix("--") {
                let (key, inline_val) = match rest.split_once('=') {
                    Some((k, v)) => (k, Some(v.to_string())),
                    None => (rest, None),
                };

                let next_arg =
                    |args: &[OsString], i: usize, flag: &str| -> Result<String, String> {
                        args.get(i)
                            .and_then(|s| s.to_str().map(str::to_string))
                            .ok_or_else(|| format!("{flag} requires an argument"))
                    };

                macro_rules! val {
                    ($flag:literal) => {
                        if let Some(v) = inline_val.clone() {
                            v
                        } else {
                            i += 1;
                            next_arg(&args, i, $flag)?
                        }
                    };
                }

                macro_rules! val_usize {
                    ($flag:literal) => {{
                        let s = val!($flag);
                        s.parse::<usize>()
                            .map_err(|_| format!("{}: invalid integer '{s}'", $flag))?
                    }};
                }

                macro_rules! val_u64 {
                    ($flag:literal) => {{
                        let s = val!($flag);
                        s.parse::<u64>()
                            .map_err(|_| format!("{}: invalid integer '{s}'", $flag))?
                    }};
                }

                match key {
                    "regexp" | "regex" => cli.patterns.push(val!("--regexp")),
                    "fixed-strings" => cli.fixed_strings = true,
                    "word-regexp" => cli.word_regexp = true,
                    "ignore-case" => cli.ignore_case = true,
                    "case-sensitive" => {} // ignored; default is case-sensitive
                    "smart-case" => cli.smart_case = true,
                    "invert-match" => cli.invert_match = true,
                    "multiline" | "multiline-dotall" => cli.multiline = true,
                    "line-number" => cli.line_number = Some(true),
                    "no-line-number" => cli.line_number = Some(false),
                    "heading" => cli.no_heading = Some(false),
                    "no-heading" => cli.no_heading = Some(true),
                    "with-filename" => cli.with_filename = Some(true),
                    "no-filename" => cli.with_filename = Some(false),
                    "only-matching" => cli.only_matching = true,
                    "files-with-matches" | "files-with-match" => cli.files_with_matches = true,
                    "files-without-match" => cli.files_without_match = true,
                    "count" => cli.count = true,
                    "count-matches" => cli.count = true,
                    "null" | "null-data" => cli.null = true,
                    "quiet" => cli.quiet = true,
                    "stats" => cli.stats = true,
                    "files" => cli.files = true,
                    "hidden" => cli.hidden = true,
                    "no-ignore" => cli.no_ignore = true,
                    "no-ignore-vcs" => cli.no_ignore_vcs = true,
                    "follow" => cli.follow = true,
                    "glob" => cli.globs.push(val!("--glob")),
                    "type"
                    | "type-not"
                    | "type-add"
                    | "type-list"
                    | "encoding"
                    | "pre"
                    | "pre-glob"
                    | "engine"
                    | "max-filesize"
                    | "sort"
                    | "sortr"
                    | "field-context-separator"
                    | "field-match-separator" => {
                        // Consume argument if not inline, then ignore
                        if inline_val.is_none() {
                            i += 1;
                        }
                    }
                    "after-context" => cli.after_context = val_usize!("--after-context"),
                    "before-context" => cli.before_context = val_usize!("--before-context"),
                    "context" => {
                        let n = val_usize!("--context");
                        cli.before_context = n;
                        cli.after_context = n;
                    }
                    "max-count" => cli.max_count = Some(val_u64!("--max-count")),
                    "max-depth" | "maxdepth" => cli.max_depth = Some(val_usize!("--max-depth")),
                    "color" | "colour" => {
                        let s = val!("--color");
                        cli.color = match s.as_str() {
                            "always" | "ansi" => ColorChoice::Always,
                            "never" => ColorChoice::Never,
                            _ => ColorChoice::Auto,
                        };
                    }
                    "version" => cli.version = true,
                    "help" => cli.help = true,
                    // Silently ignore unknown flags (ripgrep has many)
                    _ => {
                        if inline_val.is_none() && !key.starts_with("no-") {
                            // Skip potential argument for unknown flags that take a value
                            // (heuristic: flags without "no-" prefix that we don't know)
                        }
                    }
                }
            } else if let Some(short) = arg.strip_prefix('-').filter(|s| !s.is_empty()) {
                let chars: Vec<char> = short.chars().collect();
                let mut ci = 0;
                while ci < chars.len() {
                    let c = chars[ci];

                    // Short flags that consume the rest of the current arg or next arg
                    let take_val = |chars: &[char],
                                    ci: usize,
                                    args: &[OsString],
                                    i: usize,
                                    flag: &str|
                     -> Result<String, String> {
                        if ci + 1 < chars.len() {
                            Ok(chars[ci + 1..].iter().collect())
                        } else {
                            args.get(i + 1)
                                .and_then(|s| s.to_str().map(str::to_string))
                                .ok_or_else(|| format!("{flag} requires an argument"))
                        }
                    };
                    let consume_next = ci + 1 >= chars.len();

                    match c {
                        'e' => {
                            let pat = take_val(&chars, ci, &args, i, "-e")?;
                            cli.patterns.push(pat);
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'g' => {
                            let glob = take_val(&chars, ci, &args, i, "-g")?;
                            cli.globs.push(glob);
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'A' => {
                            let n = take_val(&chars, ci, &args, i, "-A")?;
                            cli.after_context = n
                                .parse()
                                .map_err(|_| format!("-A: invalid integer '{n}'"))?;
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'B' => {
                            let n = take_val(&chars, ci, &args, i, "-B")?;
                            cli.before_context = n
                                .parse()
                                .map_err(|_| format!("-B: invalid integer '{n}'"))?;
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'C' => {
                            let n = take_val(&chars, ci, &args, i, "-C")?;
                            let n: usize = n
                                .parse()
                                .map_err(|_| format!("-C: invalid integer '{n}'"))?;
                            cli.before_context = n;
                            cli.after_context = n;
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'm' => {
                            let n = take_val(&chars, ci, &args, i, "-m")?;
                            cli.max_count = Some(
                                n.parse()
                                    .map_err(|_| format!("-m: invalid integer '{n}'"))?,
                            );
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        // Flags that take the next arg and we ignore
                        't' | 'T' | 'f' | 'p' => {
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'F' => cli.fixed_strings = true,
                        'i' => cli.ignore_case = true,
                        's' => {} // case-sensitive (default)
                        'S' => cli.smart_case = true,
                        'w' => cli.word_regexp = true,
                        'v' => cli.invert_match = true,
                        'n' => cli.line_number = Some(true),
                        'N' => cli.line_number = Some(false),
                        'l' => cli.files_with_matches = true,
                        'c' => cli.count = true,
                        'o' => cli.only_matching = true,
                        'H' => cli.with_filename = Some(true),
                        'h' => cli.with_filename = Some(false),
                        'q' => cli.quiet = true,
                        '0' => cli.null = true,
                        'U' => cli.multiline = true,
                        'V' => cli.version = true,
                        'u' | 'r' | 'R' => {} // ignored
                        _ => {}               // unknown short flags: ignore
                    }
                    ci += 1;
                }
            } else {
                // Positional
                if first_positional && cli.patterns.is_empty() {
                    cli.pattern = Some(arg.to_string());
                    first_positional = false;
                } else {
                    cli.paths.push(PathBuf::from(&args[i]));
                }
            }

            i += 1;
        }

        Ok(cli)
    }

    /// Returns true if colors should be used on the given stream.
    pub fn use_color(&self, stream: &impl std::io::IsTerminal) -> bool {
        let no_color = std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty());
        match self.color {
            ColorChoice::Always => !no_color,
            ColorChoice::Never => false,
            ColorChoice::Auto => !no_color && stream.is_terminal(),
        }
    }
}
