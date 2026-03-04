// Adapted from jaq v3.0.0-beta (MIT License)
// https://github.com/01mf02/jaq
//! jq subcommand — embeds jaq as a drop-in `jq` replacement.
mod cli;
mod filter;

use cli::Cli;
use jaq_core::Vars;
use jaq_json::Val;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;

/// Run jq with the given arguments (not including argv[0]).
///
/// Entry point for both `normalize jq [args...]` and `jq -> normalize` symlink.
pub fn run_jq(args: impl Iterator<Item = OsString>) -> ExitCode {
    let mut out = io::stdout();
    let mut err = io::stderr();

    let cli = match Cli::parse(args) {
        Ok(cli) => cli,
        Err(e) => {
            let _ = writeln!(err, "Error: {e}");
            return ExitCode::from(2);
        }
    };

    if cli.version {
        let _ = writeln!(out, "jq (normalize {})", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    } else if cli.help {
        let _ = write!(out, "{}", include_str!("help.txt"));
        return ExitCode::SUCCESS;
    }

    match real_main(&cli) {
        Ok(code) => code,
        Err(e) => {
            let _ = writeln!(err, "Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn real_main(cli: &Cli) -> Result<ExitCode, String> {
    // Resolve variable bindings
    let (var_names, var_vals): (Vec<String>, Vec<Val>) =
        binds(cli).map_err(|e| e.to_string())?.into_iter().unzip();

    // Parse and compile the filter
    let (extra_vals, filter) = match &cli.filter {
        None => (Vec::new(), filter::Filter::default()),
        Some(f) => {
            let (path, code) = match f {
                cli::Filter::FromFile(path) => (
                    path.clone(),
                    std::fs::read_to_string(path).map_err(|e| e.to_string())?,
                ),
                cli::Filter::Inline(s) => (std::path::PathBuf::from("<inline>"), s.clone()),
            };
            filter::parse_compile(&path, &code, &var_names, &cli.library_path)?
        }
    };

    let mut all_vals: Vec<Val> = var_vals;
    all_vals.extend(extra_vals);
    let vars = Vars::new(all_vals);

    let color = cli.color_stdio(&io::stdout());
    let pp = cli.pp(color);

    let print = |out: &mut dyn Write, v: &Val| -> io::Result<()> {
        match v {
            Val::Str(s, _) if cli.raw_output || cli.join_output => out.write_all(s.as_ref())?,
            _ => jaq_json::write::write(out, &pp, 0, v)?,
        }
        if cli.join_output {
            out.flush()
        } else {
            writeln!(out)
        }
    };

    let last = if cli.files.is_empty() {
        let inputs = stdin_inputs(cli);
        with_stdout(|out| filter::run(&filter, vars, inputs, |v| print(out, &v)))
            .map_err(|e| e.to_string())?
    } else {
        let mut last = None;
        for file in &cli.files {
            let path = Path::new(file);
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let inputs = file_inputs(cli, &bytes);

            if cli.in_place {
                let location = path.parent().unwrap();
                let mut tmp = tempfile::Builder::new()
                    .prefix("jaq")
                    .tempfile_in(location)
                    .map_err(|e| e.to_string())?;

                last = filter::run(&filter, vars.clone(), inputs, |v| {
                    print(tmp.as_file_mut(), &v)
                })
                .map_err(|e| e.to_string())?;

                let perms = std::fs::metadata(path)
                    .map_err(|e| e.to_string())?
                    .permissions();
                tmp.persist(path).map_err(|e| e.to_string())?;
                std::fs::set_permissions(path, perms).map_err(|e| e.to_string())?;
            } else {
                last = with_stdout(|out| {
                    filter::run(&filter, vars.clone(), inputs, |v| print(out, &v))
                })
                .map_err(|e| e.to_string())?;
            }
        }
        last
    };

    if cli.exit_status {
        last.map_or_else(
            || Err("no output".to_string()),
            |b| {
                if b {
                    Ok(ExitCode::SUCCESS)
                } else {
                    Ok(ExitCode::from(1))
                }
            },
        )
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn stdin_inputs(cli: &Cli) -> Box<dyn Iterator<Item = io::Result<Val>>> {
    if cli.null_input {
        return Box::new(std::iter::once(Ok(Val::Null)));
    }
    let mut buf = Vec::new();
    let _ = io::Read::read_to_end(&mut io::stdin().lock(), &mut buf);
    if cli.raw_input {
        let s = String::from_utf8_lossy(&buf).into_owned();
        if cli.slurp {
            Box::new(std::iter::once(Ok(Val::from(s))))
        } else {
            Box::new(
                s.lines()
                    .map(|l| Ok(Val::from(l.to_owned())))
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        }
    } else if cli.slurp {
        let vals: Result<Vec<Val>, _> = jaq_json::read::parse_many(&buf)
            .map(|r| r.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string())))
            .collect();
        Box::new(std::iter::once(vals.map(|vs| vs.into_iter().collect())))
    } else {
        Box::new(
            jaq_json::read::parse_many(&buf)
                .map(|r| r.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string())))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }
}

fn file_inputs<'a>(cli: &Cli, bytes: &'a [u8]) -> Box<dyn Iterator<Item = io::Result<Val>> + 'a> {
    if cli.raw_input {
        let s = String::from_utf8_lossy(bytes).into_owned();
        if cli.slurp {
            Box::new(std::iter::once(Ok(Val::from(s))))
        } else {
            Box::new(
                s.lines()
                    .map(|l| Ok(Val::from(l.to_owned())))
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        }
    } else if cli.slurp {
        let vals: Result<Vec<Val>, _> = jaq_json::read::parse_many(bytes)
            .map(|r| r.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string())))
            .collect();
        Box::new(std::iter::once(vals.map(|vs| vs.into_iter().collect())))
    } else {
        Box::new(
            jaq_json::read::parse_many(bytes)
                .map(|r| r.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string())))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }
}

fn with_stdout<T, E>(f: impl FnOnce(&mut dyn Write) -> Result<T, E>) -> Result<T, E> {
    let stdout = io::stdout();
    if stdout.is_terminal() {
        f(&mut stdout.lock())
    } else {
        f(&mut io::BufWriter::new(stdout.lock()))
    }
}

fn binds(cli: &Cli) -> Result<Vec<(String, Val)>, String> {
    let arg = cli
        .arg
        .iter()
        .map(|(k, s)| Ok::<_, String>((k.clone(), Val::utf8_str(s.clone()))));

    let argjson = cli.argjson.iter().map(|(k, s)| {
        let v = jaq_json::read::parse_single(s.as_bytes())
            .map_err(|e| format!("--argjson {k}: {e}"))?;
        Ok((k.clone(), v))
    });

    let rawfile = cli.rawfile.iter().map(|(k, path)| {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
        Ok((k.clone(), Val::utf8_str(s)))
    });

    let slurpfile = cli.slurpfile.iter().map(|(k, path)| {
        let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
        let vals: Result<Vec<Val>, _> = jaq_json::read::parse_many(&bytes)
            .map(|r| r.map_err(|e| format!("{path:?}: {e}")))
            .collect();
        Ok((k.clone(), vals?.into_iter().collect()))
    });

    let positional: Vec<Val> = cli.args.iter().cloned().map(Val::from).collect();

    let mut var_val: Vec<(String, Val)> = arg
        .chain(rawfile)
        .chain(slurpfile)
        .chain(argjson)
        .collect::<Result<_, _>>()?;

    var_val.push(("ARGS".to_string(), make_args(&positional, &var_val)));
    let env = std::env::vars().map(|(k, v)| (k.into(), Val::from(v)));
    var_val.push(("ENV".to_string(), Val::obj(env.collect())));

    Ok(var_val)
}

fn make_args(positional: &[Val], named: &[(String, Val)]) -> Val {
    let key = |k: &str| k.to_string().into();
    let positional = positional.iter().cloned();
    let named = named
        .iter()
        .map(|(var, val)| (key(var.as_str()), val.clone()));
    let obj = [
        (key("positional"), positional.collect()),
        (key("named"), Val::obj(named.collect())),
    ];
    Val::obj(obj.into_iter().collect())
}
