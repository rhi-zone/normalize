//! Sessions management service for server-less CLI.

use super::resolve_pretty;
use crate::commands::sessions::{
    MessagesReport, PlanContent, PlansListReport, SessionListReport, SessionMode,
    SessionShowReport, SubagentsReport,
};
use crate::output::OutputFormatter;
use crate::sessions::SessionAnalysis;
use server_less::cli;
use std::cell::Cell;

/// Sessions management sub-service.
pub struct SessionsService {
    pretty: Cell<bool>,
}

impl SessionsService {
    pub fn new(pretty: &Cell<bool>) -> Self {
        Self {
            pretty: Cell::new(pretty.get()),
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
        if self.use_pretty() {
            write!(f, "{}", self.format_pretty())
        } else {
            write!(f, "{}", self.format_text())
        }
    }
}

impl std::fmt::Display for PlansListReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_text())
    }
}

impl std::fmt::Display for MessagesReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.pretty {
            write!(f, "{}", self.format_pretty())
        } else {
            write!(f, "{}", self.format_text())
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

impl SessionsService {
    fn display_analyze(&self, a: &SessionAnalysis) -> String {
        if self.pretty.get() {
            a.format_pretty()
        } else {
            a.format_text()
        }
    }
}

#[cli(
    name = "sessions",
    description = "Analyze agent session logs (Claude Code, Codex, Gemini)"
)]
impl SessionsService {
    /// List available sessions
    ///
    /// Examples:
    ///   normalize sessions list                       # list recent sessions for current project
    ///   normalize sessions list --days 7              # sessions from the last 7 days
    ///   normalize sessions list --grep "refactor"     # filter sessions by content pattern
    ///   normalize sessions list --all-projects        # show sessions across all projects
    ///   normalize sessions list --format codex        # only show Codex sessions
    ///   normalize sessions list --mode subagent       # list subagent sessions only
    ///   normalize sessions list --mode all            # list interactive + subagent sessions
    ///   normalize sessions list --agent-type Explore  # only Explore agents
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
        #[param(help = "Session mode: interactive (default), subagent, or all")] mode: Option<
            SessionMode,
        >,
        #[param(help = "Filter by agent type (e.g. Explore, general-purpose, Plan)")]
        agent_type: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SessionListReport, String> {
        let limit = limit.unwrap_or(20);
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        let is_pretty = resolve_pretty(resolved_root, pretty, compact);
        let mode = mode.unwrap_or_default();
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
            &mode,
            agent_type.as_deref(),
        )
    }

    /// Show a specific session (summary or full conversation)
    ///
    /// Examples:
    ///   normalize sessions show abc123               # show session summary (fuzzy match on ID)
    ///   normalize sessions show abc123 --full        # show full conversation log
    ///   normalize sessions show abc123 --exact       # require exact ID match
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &self,
        #[param(positional, help = "Session ID or path")] session: String,
        #[param(help = "Show full conversation log")] full: bool,
        #[param(help = "Require exact/prefix match (disable fuzzy)")] exact: bool,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(help = "Filter by specific project path")] project: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        pretty: bool,
        compact: bool,
    ) -> Result<SessionShowReport, String> {
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        let is_pretty = super::resolve_pretty(resolved_root, pretty, compact);
        let effective_project = project_path.or(root_path);
        let report = crate::commands::sessions::build_show_report(
            &session,
            effective_project,
            format.as_deref(),
            full,
            exact,
        )?;
        Ok(report.with_pretty(is_pretty))
    }

    /// Run deep behavioral analysis on a session (tool stats, errors, token costs, corrections)
    ///
    /// Examples:
    ///   normalize sessions analyze abc123             # analyze a session by ID
    ///   normalize sessions analyze abc123 --pretty    # colored terminal output
    ///   normalize sessions analyze abc123 --json      # machine-readable analysis
    ///   normalize sessions analyze agent-abc --mode subagent  # analyze a subagent
    #[cli(display_with = "display_analyze")]
    #[allow(clippy::too_many_arguments)]
    pub fn analyze(
        &self,
        #[param(positional, help = "Session ID or pattern")] session: String,
        #[param(help = "Require exact/prefix match (disable fuzzy)")] exact: bool,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(help = "Filter by specific project path")] project: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Session mode: interactive (default), subagent, or all")] mode: Option<
            SessionMode,
        >,
        #[param(help = "Filter by agent type (e.g. Explore, general-purpose, Plan)")]
        agent_type: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SessionAnalysis, String> {
        let _mode = mode; // session resolution already searches subagents
        let _agent_type = agent_type; // session resolution already searches subagents
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        self.pretty
            .set(super::resolve_pretty(resolved_root, pretty, compact));
        let effective_project = project_path.or(root_path);
        crate::commands::sessions::build_analyze_report(
            &session,
            effective_project,
            format.as_deref(),
            exact,
        )
    }

    /// Show aggregate statistics across sessions
    ///
    /// Examples:
    ///   normalize sessions stats                             # aggregate stats for recent sessions
    ///   normalize sessions stats --days 30                   # stats for the last 30 days
    ///   normalize sessions stats --group-by project          # group results by project
    ///   normalize sessions stats --group-by project,day      # group by project and day
    ///   normalize sessions stats --mode subagent             # stats for subagent sessions only
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
        #[param(help = "Group results by comma-separated fields: project, day (e.g. project,day)")]
        group_by: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Session mode: interactive (default), subagent, or all")] mode: Option<
            SessionMode,
        >,
        #[param(help = "Filter by agent type (e.g. Explore, general-purpose, Plan)")]
        agent_type: Option<String>,
    ) -> Result<SessionAnalysis, String> {
        let limit = limit.unwrap_or(20);
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let mode = mode.unwrap_or_default();

        // When group_by is specified, delegate to the grouped command path which prints
        // per-group output directly. This uses process::exit to avoid double-printing
        // from the service framework.
        if let Some(ref group_by_str) = group_by {
            let group_by_fields: Vec<String> = group_by_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let exit_code = crate::commands::sessions::show_stats_grouped(
                root_path,
                limit,
                format.as_deref(),
                grep.as_deref(),
                days,
                since.as_deref(),
                until.as_deref(),
                project_path,
                all_projects,
                &group_by_fields,
                &mode,
                agent_type.as_deref(),
            );
            std::process::exit(exit_code);
        }

        crate::commands::sessions::build_stats_data(
            root_path,
            limit,
            format.as_deref(),
            grep.as_deref(),
            days,
            since.as_deref(),
            until.as_deref(),
            project_path,
            all_projects,
            &mode,
            agent_type.as_deref(),
        )
    }

    /// Extract all messages across sessions into a flat, queryable form
    ///
    /// Examples:
    ///   normalize sessions messages                                # user messages from recent sessions
    ///   normalize sessions messages --role all                     # all roles (user + assistant)
    ///   normalize sessions messages --grep "error" --no-truncate   # search messages, full text
    ///   normalize sessions messages --grep "panic" --context 2     # matching lines with 2 lines of context
    ///   normalize sessions messages --show-usage --sort-by-tokens  # heaviest turns first
    #[allow(clippy::too_many_arguments)]
    pub fn messages(
        &self,
        #[param(help = "Filter by role: user (default), assistant, tool, system, all")]
        role: Option<crate::commands::sessions::messages::RoleFilter>,
        #[param(help = "Filter messages by content pattern")] grep: Option<String>,
        #[param(help = "Filter sessions from the last N days")] days: Option<u32>,
        #[param(help = "Filter sessions since date (YYYY-MM-DD)")] since: Option<String>,
        #[param(help = "Filter sessions until date (YYYY-MM-DD)")] until: Option<String>,
        #[param(help = "Filter by specific project path")] project: Option<String>,
        #[param(help = "Show sessions from all projects")] all_projects: bool,
        #[param(help = "Filter to a specific session ID")] session: Option<String>,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(short = 'n', help = "Maximum number of sessions")] limit: Option<usize>,
        #[param(help = "Show per-turn token usage (input/output/cache)")] show_usage: bool,
        #[param(help = "Sort by descending token count (heaviest turns first)")]
        sort_by_tokens: bool,
        #[param(
            short = 'C',
            help = "Lines of context around each matching line (requires --grep)"
        )]
        context: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Session mode: interactive (default), subagent, or all")] mode: Option<
            SessionMode,
        >,
        #[param(help = "Filter by agent type (e.g. Explore, general-purpose, Plan)")]
        agent_type: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<MessagesReport, String> {
        let limit = limit.unwrap_or(20);
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        let is_pretty = resolve_pretty(resolved_root, pretty, compact);
        let context_lines = context.unwrap_or(0);
        if context_lines > 0 && grep.is_none() {
            return Err("--context requires --grep".to_string());
        }
        let mode = mode.unwrap_or_default();
        crate::commands::sessions::build_messages_report(
            root_path,
            limit,
            format.as_deref(),
            role.unwrap_or_default(),
            grep.as_deref(),
            days,
            since.as_deref(),
            until.as_deref(),
            project_path,
            all_projects,
            session.as_deref(),
            show_usage,
            sort_by_tokens,
            context_lines,
            is_pretty,
            &mode,
            agent_type.as_deref(),
        )
    }

    /// List subagents for a given parent session
    ///
    /// Examples:
    ///   normalize sessions subagents abc123            # list subagents of session abc123
    ///   normalize sessions subagents abc123 --json     # machine-readable output
    pub fn subagents(
        &self,
        #[param(positional, help = "Parent session ID")] session: String,
        #[param(help = "Force specific format: claude, codex, gemini, normalize")] format: Option<
            String,
        >,
        #[param(help = "Filter by specific project path")] project: Option<String>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
    ) -> Result<SubagentsReport, String> {
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let effective_project = project_path.or(root_path);
        crate::commands::sessions::subagents::build_subagents_report(
            &session,
            effective_project,
            format.as_deref(),
        )
    }

    /// List and view agent plans
    ///
    /// Examples:
    ///   normalize sessions plans                     # list all saved plans
    ///   normalize sessions plans my-plan             # view a specific plan by name
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
