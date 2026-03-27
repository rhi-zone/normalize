//! Sessions management service for server-less CLI.

use super::resolve_pretty;
use crate::commands::sessions::{
    MessagesReport, PatternsReport, PlanContent, PlansListReport, SessionListReport, SessionMode,
    SessionShowReport, SubagentsReport,
};
use crate::output::OutputFormatter;
use crate::sessions::SessionAnalysisReport;
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

/// Report type for plans (list or content).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "kind")]
pub enum PlansReport {
    List(PlansListReport),
    Content(PlanContent),
}

impl crate::output::OutputFormatter for PlansReport {
    fn format_text(&self) -> String {
        match self {
            Self::List(r) => r.format_text(),
            Self::Content(c) => c.to_string(),
        }
    }
}

impl SessionsService {
    fn display_analyze(&self, a: &SessionAnalysisReport) -> String {
        if self.pretty.get() {
            a.format_pretty()
        } else {
            a.format_text()
        }
    }

    /// Generic display bridge: delegates to the report's own pretty flag.
    fn display_output<T: OutputFormatter>(&self, value: &T) -> String {
        if self.pretty.get() {
            value.format_pretty()
        } else {
            value.format_text()
        }
    }
}

#[cli(
    name = "sessions",
    description = "Analyze agent session logs (Claude Code, Codex, Gemini)",
    global = [
        pretty = "Human-friendly output with colors and formatting",
        compact = "Compact output without colors (overrides TTY detection)",
    ]
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
    ///   normalize sessions list --sort duration       # longest sessions first
    ///   normalize sessions list --sort +name          # alphabetical by session name
    #[cli(display_with = "display_output")]
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
        #[param(
            help = "Sort keys (comma-separated, prefix with - for desc or + for asc): date, duration, name. E.g. duration, +name, -date"
        )]
        sort: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<SessionListReport, String> {
        let limit = limit.unwrap_or(20);
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        self.pretty
            .set(resolve_pretty(resolved_root, pretty, compact));
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
            &mode,
            agent_type.as_deref(),
            sort.as_deref(),
        )
    }

    /// Show a specific session (summary or full conversation)
    ///
    /// Examples:
    ///   normalize sessions show abc123               # show session summary (fuzzy match on ID)
    ///   normalize sessions show abc123 --full        # show full conversation log
    ///   normalize sessions show abc123 --exact       # require exact ID match
    #[cli(display_with = "display_output")]
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
        self.pretty
            .set(super::resolve_pretty(resolved_root, pretty, compact));
        let effective_project = project_path.or(root_path);
        crate::commands::sessions::build_show_report(
            &session,
            effective_project,
            format.as_deref(),
            full,
            exact,
        )
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
    ) -> Result<SessionAnalysisReport, String> {
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
    ///   normalize sessions stats --sort name                 # sort tool rows alphabetically
    ///   normalize sessions stats --sort errors               # sort tool rows by error count
    #[cli(display_with = "display_output")]
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
        #[param(
            short = 'n',
            help = "Maximum number of sessions (0 = all, default: all)"
        )]
        limit: Option<usize>,
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
        #[param(
            help = "Sort tool rows (comma-separated, prefix with - for desc or + for asc): calls, errors, name. E.g. name, -errors"
        )]
        sort: Option<String>,
    ) -> Result<SessionAnalysisReport, String> {
        let limit = limit.unwrap_or(0);
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
            sort.as_deref(),
        )
    }

    /// Extract all messages across sessions into a flat, queryable form
    ///
    /// Examples:
    ///   normalize sessions messages                                # user messages from recent sessions
    ///   normalize sessions messages --role all                     # all roles (user + assistant)
    ///   normalize sessions messages --grep "error" --no-truncate   # search messages, full text
    ///   normalize sessions messages --grep "panic" --context 2     # matching lines with 2 lines of context
    ///   normalize sessions messages --show-usage --sort -tokens    # heaviest turns first
    ///   normalize sessions messages --sort timestamp               # chronological across sessions
    ///   normalize sessions messages --sort +session,-tokens        # by session asc, then tokens desc
    #[cli(display_with = "display_output")]
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
        #[param(
            help = "Sort keys (comma-separated, prefix with - for desc or + for asc): tokens, timestamp, session. E.g. -tokens, +session,-tokens, timestamp"
        )]
        sort: Option<String>,
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
        self.pretty
            .set(resolve_pretty(resolved_root, pretty, compact));
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
            sort.as_deref(),
            context_lines,
            &mode,
            agent_type.as_deref(),
        )
    }

    /// List subagents for a given parent session
    ///
    /// Examples:
    ///   normalize sessions subagents abc123            # list subagents of session abc123
    ///   normalize sessions subagents abc123 --json     # machine-readable output
    #[cli(display_with = "display_output")]
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

    /// Analyze tool call sequence patterns across sessions using Markov chain transition matrices
    ///
    /// Examples:
    ///   normalize sessions patterns                         # analyze tool patterns for recent sessions
    ///   normalize sessions patterns --days 30               # patterns for the last 30 days
    ///   normalize sessions patterns --mode subagent         # patterns for subagent sessions only
    ///   normalize sessions patterns --all-projects          # patterns across all projects
    ///   normalize sessions patterns --sort tool_count       # sort outliers by tool usage
    ///   normalize sessions patterns --sort +path            # sort outliers alphabetically by path
    #[cli(display_with = "display_output")]
    #[allow(clippy::too_many_arguments)]
    pub fn patterns(
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
        #[param(
            short = 'n',
            help = "Maximum number of sessions (0 = all, default: all)"
        )]
        limit: Option<usize>,
        #[param(short = 'r', help = "Root directory (defaults to current directory)")] root: Option<
            String,
        >,
        #[param(help = "Session mode: interactive (default), subagent, or all")] mode: Option<
            SessionMode,
        >,
        #[param(help = "Filter by agent type (e.g. Explore, general-purpose, Plan)")]
        agent_type: Option<String>,
        #[param(
            help = "Sort outlier rows (comma-separated, prefix with - for desc or + for asc): divergence, tool_count, turn_count, path. E.g. tool_count, +path"
        )]
        sort: Option<String>,
        pretty: bool,
        compact: bool,
    ) -> Result<PatternsReport, String> {
        let limit = limit.unwrap_or(0);
        let root_path = root.as_deref().map(std::path::Path::new);
        let project_path = project.as_deref().map(std::path::Path::new);
        let resolved_root = root_path.unwrap_or(std::path::Path::new("."));
        self.pretty
            .set(resolve_pretty(resolved_root, pretty, compact));
        let mode = mode.unwrap_or_default();
        crate::commands::sessions::build_patterns_report(
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
            sort.as_deref(),
        )
    }

    /// List and view agent plans
    ///
    /// Examples:
    ///   normalize sessions plans                     # list all saved plans
    ///   normalize sessions plans my-plan             # view a specific plan by name
    #[cli(display_with = "display_output")]
    pub fn plans(
        &self,
        #[param(positional, help = "Plan name to view (omit to list all)")] name: Option<String>,
        #[param(short = 'n', help = "Maximum number of plans")] limit: Option<usize>,
    ) -> Result<PlansReport, String> {
        let limit = limit.unwrap_or(20);
        match name {
            Some(ref n) => {
                let content = crate::commands::sessions::build_plan_content(n)?;
                Ok(PlansReport::Content(content))
            }
            None => {
                let list = crate::commands::sessions::build_plans_list(limit)?;
                Ok(PlansReport::List(list))
            }
        }
    }
}
