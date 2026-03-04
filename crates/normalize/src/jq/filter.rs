// Adapted from jaq v3.0.0-beta (MIT License)
// https://github.com/01mf02/jaq
//! Filter parsing, compilation, and execution.
use jaq_core::compile::Compiler;
use jaq_core::load::{Arena, File, Loader, import};
use jaq_core::{Ctx, Vars, data::JustLut};
use jaq_json::Val;
use std::{
    io,
    path::{Path, PathBuf},
};

pub type Filter = jaq_core::Filter<JustLut<Val>>;

pub fn parse_compile(
    path: &Path,
    code: &str,
    vars: &[String],
    paths: &[PathBuf],
) -> Result<(Vec<Val>, Filter), String> {
    let default = ["~/.jq", "$ORIGIN/../lib/jq", "$ORIGIN/../lib"].map(|x: &str| x.into());
    let paths = if paths.is_empty() { &default } else { paths };

    let vars: Vec<_> = vars.iter().map(|v| format!("${v}")).collect();
    let arena = Arena::default();
    let defs = jaq_std::defs().chain(jaq_json::defs());
    let loader = Loader::new(defs).with_std_read(paths);
    let modules = loader
        .load(
            &arena,
            File {
                path: path.to_path_buf(),
                code,
            },
        )
        .map_err(|errs| format_load_errors(&errs))?;

    let mut vals = Vec::new();
    import(&modules, |p| {
        let path = p.find(paths, "json")?;
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let v: Val = jaq_json::read::parse_many(&bytes)
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        vals.push(v);
        Ok(())
    })
    .map_err(|errs| format_load_errors(&errs))?;

    let filter = Compiler::default()
        .with_funs(jaq_std::funs::<JustLut<Val>>().chain(jaq_json::funs::<JustLut<Val>>()))
        .with_global_vars(vars.iter().map(|v| v.as_str()))
        .compile(modules)
        .map_err(|errs| format_compile_errors(&errs))?;

    Ok((vals, filter))
}

fn format_load_errors<S: core::fmt::Debug, P: core::fmt::Debug>(
    errs: &jaq_core::load::Errors<S, P>,
) -> String {
    errs.iter()
        .flat_map(|(file, err)| {
            let prefix = format!("[{:?}] ", file.path);
            match err {
                jaq_core::load::Error::Io(es) => es
                    .iter()
                    .map(|(p, e)| format!("{prefix}{p:?}: {e}"))
                    .collect(),
                jaq_core::load::Error::Lex(es) => es
                    .iter()
                    .map(|(_, found)| format!("{prefix}lex error near {found:?}"))
                    .collect(),
                jaq_core::load::Error::Parse(es) => es
                    .iter()
                    .map(|(exp, found)| {
                        format!("{prefix}parse error: expected {exp:?}, found {found:?}")
                    })
                    .collect::<Vec<_>>(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_compile_errors<S: core::fmt::Debug, P: core::fmt::Debug>(
    errs: &jaq_core::compile::Errors<S, P>,
) -> String {
    errs.iter()
        .flat_map(|(file, es)| {
            let prefix = format!("[{:?}] ", file.path);
            es.iter()
                .map(|(_, undef)| format!("{prefix}compile error: undefined {undef:?}"))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn val_is_truthy(v: &Val) -> bool {
    !matches!(v, Val::Null | Val::Bool(false))
}

/// Run a filter with given inputs, calling `f` for every output value.
pub fn run(
    filter: &Filter,
    vars: Vars<Val>,
    inputs: impl Iterator<Item = io::Result<Val>>,
    mut f: impl FnMut(Val) -> io::Result<()>,
) -> Result<Option<bool>, RunError> {
    let mut last = None;
    for input in inputs {
        let input = input.map_err(RunError::Io)?;
        let ctx = Ctx::<JustLut<Val>>::new(&filter.lut, vars.clone());
        for output in filter.id.run((ctx, input)) {
            match output {
                Ok(v) => {
                    last = Some(val_is_truthy(&v));
                    f(v).map_err(RunError::Io)?;
                }
                Err(e) => eprintln!("jq error: {e:?}"),
            }
        }
    }
    Ok(last)
}

#[derive(Debug)]
pub enum RunError {
    Io(io::Error),
}

impl core::fmt::Display for RunError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<io::Error> for RunError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
