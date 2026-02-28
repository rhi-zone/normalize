//! Sessions management service for server-less CLI.

use crate::commands::sessions::{
    PlanContent, PlansListReport, SessionListReport, SessionShowReport,
};
use crate::output::OutputFormatter;
use crate::sessions::SessionAnalysis;
use server_less::cli;
use std::cell::Cell;

/// Sessions management sub-service.
pub struct SessionsService {
    _pretty: Cell<bool>,
}

impl SessionsService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            _pretty: Cell::new(pretty.get()),
        }
    }
}

impl std::fmt::Display for SessionListReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.use_pretty() {
            write!(f, "{}", self.format_pretty())
        } else {
            write!(f, "{}", self.format_text())
        }
    }
}

impl std::fmt::Display for SessionShowReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

impl std::fmt::Display for PlansListReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

/// Output type for sessions show (report or analysis).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum SessionShowOutput {
    Report(SessionShowReport),
    Analysis(SessionAnalysis),
}

impl std::fmt::Display for SessionShowOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Report(r) => write!(f, "{}", r.format_text()),
            Self::Analysis(a) => write!(f, "{}", a.format_text()),
        }
    }
}

/// Output type for plans (list or content).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum PlansOutput {
    List(PlansListReport),
    Content(PlanContent),
}

impl std::fmt::Display for PlansOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::List(r) => write!(f, "{}", r.format_text()),
            Self::Content(c) => write!(f, "{}", c),
        }
    }
}

#[cli(
    name = "sessions",
    about = "Analyze agent session logs (Claude Code, Codex, Gemini)"
)]
impl SessionsService {
    /// List available sessions
    #[allow(clippy::too_many_arguments)]
    pub fn list(
        &self,
        #[param(help = "Filter sessions by grep pattern")] grep: Option<String>,
        #[param(help = "Filter sessions from the last N days")] days: Option<u32>,
        #[param(help = "Filter sessions since date (YYYY-MM-DD)")] since: Option<String>,
        #[param(help = "Filter sessions until date (YYYY-MM-DD)")] until: Option<String>,
        #[param(help = "Filter by specific project path")] project: Option<String>,
        #[param(help = "Show sessions from all projects")] all_projects: bool,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(short = 'n', help = "Maximum number of sessions")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<SessionListReport, String> {
        let limit = limit.unwrap_or(20);
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        let config = crate::config::NormalizeConfig::load(resolved_root);
        let is_pretty = !compact && (pretty || config.pretty.enabled());
        crate::commands::sessions::build_session_list(
            root_path,
            limit,
            format.as_deref(),
            grep.as_deref(),
            days,
            since.as_deref(),
            until.as_deref(),
            project_path,
            all_projects,
            is_pretty,
        )
    }

    /// Show a specific session (summary or full conversation)
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &self,
        #[param(positional, help = "Session ID or path")] session: String,
        #[param(help = "Run full analysis instead of summary")] analyze: bool,
        #[param(help = "Show full conversation log")] full: bool,
        #[param(help = "Require exact/prefix match (disable fuzzy)")] exact: bool,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<SessionShowOutput, String> {
        let root_path = root.as_deref().map(std::path::Path::new);
        if analyze {
            let analysis = crate::commands::sessions::build_analyze_report(
                &session,
                root_path,
                format.as_deref(),
                exact,
            )?;
            Ok(SessionShowOutput::Analysis(analysis))
        } else {
            let report = crate::commands::sessions::build_show_report(
                &session,
                root_path,
                format.as_deref(),
                full,
                exact,
            )?;
            Ok(SessionShowOutput::Report(report))
        }
    }

    /// Show aggregate statistics across sessions
    #[allow(clippy::too_many_arguments)]
    pub fn stats(
        &self,
        #[param(help = "Filter sessions by grep pattern")] grep: Option<String>,
        #[param(help = "Filter sessions from the last N days")] days: Option<u32>,
        #[param(help = "Filter sessions since date (YYYY-MM-DD)")] since: Option<String>,
        #[param(help = "Filter sessions until date (YYYY-MM-DD)")] until: Option<String>,
        #[param(help = "Filter by specific project path")] project: Option<String>,
        #[param(help = "Show sessions from all projects")] all_projects: bool,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(short = 'n', help = "Maximum number of sessions")] limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<SessionAnalysis, String> {
        let limit = limit.unwrap_or(20);
        crate::commands::sessions::build_stats_data(
            root.as_deref().map(std::path::Path::new),
            limit,
            format.as_deref(),
            grep.as_deref(),
            days,
            since.as_deref(),
            until.as_deref(),
            project.as_deref().map(std::path::Path::new),
            all_projects,
        )
    }

    /// List and view agent plans
    pub fn plans(
        &self,
        #[param(positional, help = "Plan name to view (omit to list all)")] name: Option<String>,
        #[param(short = 'n', help = "Maximum number of plans")] limit: Option<usize>,
    ) -> Result<PlansOutput, String> {
        let limit = limit.unwrap_or(20);
        match name {
            Some(ref n) => {
                let content = crate::commands::sessions::build_plan_content(n)?;
                Ok(PlansOutput::Content(content))
            }
            None => {
                let list = crate::commands::sessions::build_plans_list(limit)?;
                Ok(PlansOutput::List(list))
            }
        }
    }
}
