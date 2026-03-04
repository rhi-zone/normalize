// ast-grep subcommand — CLI argument parsing.
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Cli {
    pub pattern: Option<String>,
    pub lang: Option<String>,
    pub json: bool,
    pub files_with_matches: bool,
    pub no_filename: bool,
    pub hidden: bool,
    pub no_ignore: bool,
    pub follow: bool,
    pub max_depth: Option<usize>,
    pub paths: Vec<PathBuf>,
    pub version: bool,
    pub help: bool,
}

impl Cli {
    pub fn parse(iter: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut cli = Self::default();
        let args: Vec<OsString> = iter.collect();
        let mut i = 0;

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

                let next_str =
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
                            next_str(&args, i, $flag)?
                        }
                    };
                }

                match key {
                    "pattern" => cli.pattern = Some(val!("--pattern")),
                    "lang" | "language" => cli.lang = Some(val!("--lang")),
                    "json" => cli.json = true,
                    "files-with-matches" | "files-with-match" => cli.files_with_matches = true,
                    "no-filename" => cli.no_filename = true,
                    "hidden" => cli.hidden = true,
                    "no-ignore" => cli.no_ignore = true,
                    "follow" => cli.follow = true,
                    "max-depth" => {
                        let s = val!("--max-depth");
                        cli.max_depth = Some(
                            s.parse::<usize>()
                                .map_err(|_| format!("--max-depth: invalid integer '{s}'"))?,
                        );
                    }
                    "version" => cli.version = true,
                    "help" => cli.help = true,
                    // Common ast-grep flags we don't support but accept silently
                    "rewrite" | "rule" | "config" | "color" | "color-output" | "stdin"
                    | "interactive" | "update-all" | "format" => {
                        if inline_val.is_none() {
                            i += 1;
                        }
                    }
                    _ => {} // unknown flags: ignore
                }
            } else if let Some(short) = arg.strip_prefix('-').filter(|s| !s.is_empty()) {
                let chars: Vec<char> = short.chars().collect();
                let mut ci = 0;
                while ci < chars.len() {
                    let c = chars[ci];

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
                        'p' => {
                            let pat = take_val(&chars, ci, &args, i, "-p")?;
                            cli.pattern = Some(pat);
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'l' => {
                            let lang = take_val(&chars, ci, &args, i, "-l")?;
                            cli.lang = Some(lang);
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'r' => {
                            // --rewrite: not supported, skip arg
                            if consume_next {
                                i += 1;
                            }
                            break;
                        }
                        'j' => cli.json = true,
                        // 'l' already handled above
                        'V' => cli.version = true,
                        _ => {}
                    }
                    ci += 1;
                }
            } else {
                cli.paths.push(PathBuf::from(arg));
            }

            i += 1;
        }

        Ok(cli)
    }
}
