// Adapted from jaq v3.0.0-beta (MIT License)
// https://github.com/01mf02/jaq
//! Filter parsing, compilation, and execution.
use super::Error;
use jaq_all::data::{Filter, Runner};
use jaq_all::jaq_core::{ValT, compile, load};
use jaq_all::json::Val;
use jaq_all::load::{FileReports, compile_errors, load_errors};
use std::{io, path::PathBuf};

pub fn parse_compile(
    path: &PathBuf,
    code: &str,
    vars: &[String],
    paths: &[PathBuf],
) -> Result<(Vec<Val>, Filter), Vec<FileReports<PathBuf>>> {
    use compile::Compiler;
    use load::{Arena, File, Loader, import};

    let default = ["~/.jq", "$ORIGIN/../lib/jq", "$ORIGIN/../lib"].map(|x| x.into());
    let paths = if paths.is_empty() { &default } else { paths };

    let vars: Vec<_> = vars.iter().map(|v| format!("${v}")).collect();
    let arena = Arena::default();
    let loader = Loader::new(jaq_all::defs()).with_std_read(paths);
    let path = path.into();
    let modules = loader
        .load(&arena, File { path, code })
        .map_err(load_errors)?;

    let mut vals = Vec::new();
    import(&modules, |p| {
        let path = p.find(paths, "json")?;
        vals.push(jaq_all::fmts::read::json_array(path).map_err(|e| e.to_string())?);
        Ok(())
    })
    .map_err(load_errors)?;

    let compiler = Compiler::default()
        .with_funs(jaq_all::data::funs())
        .with_global_vars(vars.iter().map(|v| &**v));
    let filter = compiler.compile(modules).map_err(compile_errors)?;
    Ok((vals, filter))
}

/// Run a filter with given input values and call `f` for every output value.
pub(crate) fn run(
    runner: &Runner,
    filter: &Filter,
    vars: jaq_all::jaq_core::Vars<Val>,
    inputs: impl Iterator<Item = io::Result<Val>>,
    mut f: impl FnMut(Val) -> io::Result<()>,
) -> Result<Option<bool>, Error> {
    let mut last = None;
    jaq_all::data::run(runner, filter, vars, inputs, Error::Parse, |v| {
        let v = v.map_err(Error::Jaq)?;
        last = Some(v.as_bool());
        f(v).map_err(Into::into)
    })?;
    Ok(last)
}
