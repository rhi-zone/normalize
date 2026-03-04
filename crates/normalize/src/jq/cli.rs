// Adapted from jaq v3.0.0-beta (MIT License)
// https://github.com/01mf02/jaq
//! Command-line argument parsing
use core::fmt;
use std::ffi::OsString;
use std::path::PathBuf;

pub use jaq_all::fmts::Format;

#[derive(Debug, Default)]
pub struct Cli {
    // Input options
    pub null_input: bool,
    pub raw_input: bool,
    pub slurp: bool,

    // Output options
    pub compact_output: bool,
    pub raw_output: bool,
    pub join_output: bool,
    pub in_place: bool,
    pub sort_keys: bool,
    pub color_output: bool,
    pub monochrome_output: bool,
    pub tab: bool,
    pub indent: usize,

    // Format options
    pub from: Option<Format>,
    pub to: Option<Format>,

    // Compilation options
    pub from_file: bool,
    pub library_path: Vec<PathBuf>,

    // Key-value options
    pub arg: Vec<(String, String)>,
    pub argjson: Vec<(String, String)>,
    pub slurpfile: Vec<(String, OsString)>,
    pub rawfile: Vec<(String, OsString)>,

    // Positional arguments
    pub filter: Option<Filter>,
    pub files: Vec<String>,
    pub args: Vec<String>,
    pub run_tests: Option<Vec<PathBuf>>,
    pub exit_status: bool,
    pub version: bool,
    pub help: bool,
}

#[derive(Debug)]
pub enum Filter {
    Inline(String),
    FromFile(PathBuf),
}

impl Cli {
    fn positional(&mut self, mode: &Mode, arg: OsString) -> Result<(), Error> {
        if self.filter.is_none() {
            self.filter = Some(if self.from_file {
                Filter::FromFile(arg.into())
            } else {
                Filter::Inline(arg.into_string()?)
            })
        } else {
            match mode {
                Mode::Files => self.files.push(arg.into_string()?),
                Mode::Args => self.args.push(arg.into_string()?),
            }
        }
        Ok(())
    }

    fn long(
        &mut self,
        mode: &mut Mode,
        arg: &str,
        args: &mut impl Iterator<Item = OsString>,
    ) -> Result<(), Error> {
        let int = |s: OsString| s.into_string().ok()?.parse().ok();
        let fmt = |s: OsString| -> Result<Format, Error> {
            let s = s.into_string().map_err(Error::Utf8)?;
            Format::parse(&s).ok_or(Error::Format(s))
        };
        match arg {
            "" => args.try_for_each(|arg| self.positional(mode, arg))?,

            "null-input" => self.short('n', args)?,
            "raw-input" => self.short('R', args)?,
            "slurp" => self.short('s', args)?,

            "compact-output" => self.short('c', args)?,
            "raw-output" => self.short('r', args)?,
            "join-output" => self.short('j', args)?,
            "in-place" => self.short('i', args)?,
            "sort-keys" => self.short('S', args)?,
            "color-output" => self.short('C', args)?,
            "monochrome-output" => self.short('M', args)?,
            "tab" => self.tab = true,
            "indent" => self.indent = args.next().and_then(int).ok_or(Error::Int("--indent"))?,

            "from" => self.from = Some(fmt(args.next().ok_or(Error::Path("--from"))?)?),
            "to" => self.to = Some(fmt(args.next().ok_or(Error::Path("--to"))?)?),

            "from-file" => self.short('f', args)?,
            "library-path" => self.short('L', args)?,
            "arg" => {
                let (name, value) = parse_key_val("--arg", args)?;
                self.arg.push((name, value.into_string()?));
            }
            "argjson" => {
                let (name, value) = parse_key_val("--argjson", args)?;
                self.argjson.push((name, value.into_string()?));
            }
            "slurpfile" => self.slurpfile.push(parse_key_val("--slurpfile", args)?),
            "rawfile" => self.rawfile.push(parse_key_val("--rawfile", args)?),

            "args" => *mode = Mode::Args,
            "run-tests" => self.run_tests = Some(args.map(PathBuf::from).collect()),
            "exit-status" => self.short('e', args)?,
            "version" => self.short('V', args)?,
            "help" => self.short('h', args)?,

            arg => Err(Error::Flag(format!("--{arg}")))?,
        }
        Ok(())
    }

    fn short(
        &mut self,
        arg: char,
        _args: &mut impl Iterator<Item = OsString>,
    ) -> Result<(), Error> {
        match arg {
            'n' => self.null_input = true,
            'R' => self.raw_input = true,
            's' => self.slurp = true,

            'c' => self.compact_output = true,
            'r' => self.raw_output = true,
            'j' => self.join_output = true,
            'i' => self.in_place = true,
            'S' => self.sort_keys = true,
            'C' => self.color_output = true,
            'M' => self.monochrome_output = true,

            'f' => self.from_file = true,
            'L' => self
                .library_path
                .push(_args.next().ok_or(Error::Path("-L"))?.into()),
            'e' => self.exit_status = true,
            'V' => self.version = true,
            'h' => self.help = true,
            arg => Err(Error::Flag(format!("-{arg}")))?,
        }
        Ok(())
    }

    pub fn parse(iter: impl Iterator<Item = OsString>) -> Result<Self, Error> {
        let mut cli = Self {
            indent: 2,
            ..Self::default()
        };
        let mut mode = Mode::Files;
        let mut args = iter;
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some(s) => match s.strip_prefix("--") {
                    Some(rest) => cli.long(&mut mode, rest, &mut args)?,
                    None => match s.strip_prefix('-') {
                        Some(rest) if !rest.is_empty() => {
                            rest.chars().try_for_each(|c| cli.short(c, &mut args))?
                        }
                        _ => cli.positional(&mode, arg)?,
                    },
                },
                None => cli.positional(&mode, arg)?,
            }
        }
        Ok(cli)
    }

    pub fn color_stdio(&self, stream: &impl std::io::IsTerminal) -> bool {
        let no_color = std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty());
        if self.monochrome_output || no_color {
            false
        } else if self.color_output {
            true
        } else {
            stream.is_terminal()
        }
    }

    pub fn indent(&self) -> String {
        if self.tab {
            "\t".to_string()
        } else {
            " ".repeat(self.indent)
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Flag(String),
    Utf8(OsString),
    KeyValue(&'static str),
    Int(&'static str),
    Path(&'static str),
    Format(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Flag(s) => write!(f, "unknown flag: {s}"),
            Self::Utf8(s) => write!(f, "invalid UTF-8: {s:?}"),
            Self::KeyValue(o) => write!(f, "{o} expects a key and a value"),
            Self::Int(o) => write!(f, "{o} expects an integer"),
            Self::Path(o) => write!(f, "{o} expects a path"),
            Self::Format(s) => write!(f, "unknown format: {s}"),
        }
    }
}

impl From<OsString> for Error {
    fn from(e: OsString) -> Self {
        Self::Utf8(e)
    }
}

fn parse_key_val(
    arg: &'static str,
    args: &mut impl Iterator<Item = OsString>,
) -> Result<(String, OsString), Error> {
    let err = || Error::KeyValue(arg);
    let key = args.next().ok_or_else(err)?.into_string()?;
    let val = args.next().ok_or_else(err)?;
    Ok((key, val))
}

enum Mode {
    Args,
    Files,
}
