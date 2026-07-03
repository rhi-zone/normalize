//! Output formatting utilities (re-exported from normalize-output).

pub use normalize_output::*;

use normalize_rank::ranked::{RankEntry, RiskTier, format_ranked_table};
use nu_ansi_term::{Color, Style};

/// Map a [`RiskTier`] to its house-style color for `format_pretty()` output.
///
/// This is the single mapping from severity to color across all rank
/// subcommands — complexity, length, and test-gaps all route their tier
/// coloring through here so the palette stays consistent. Keeping the
/// `nu_ansi_term` dependency here (rather than in `normalize-rank`) lets the
/// library crate stay color-free; consumers ask for the color by tier.
pub fn tier_color(tier: RiskTier) -> Color {
    match tier {
        RiskTier::Critical => Color::Red,
        RiskTier::High => Color::Yellow,
        RiskTier::Moderate => Color::Blue,
        RiskTier::Low => Color::Green,
    }
}

/// House-style `format_pretty()` for any [`RankEntry`] table.
///
/// Renders the same table as [`format_ranked_table`] (so column widths match
/// text mode exactly), then applies nu_ansi_term styling: the `#` title is
/// bolded, and each data row is colored by `row_color(entry)` if it returns
/// `Some`. Coloring whole rows (rather than individual cells) keeps the
/// `format_ranked_table` width math correct — ANSI escapes wrap the
/// already-padded line and never enter the width computation.
///
/// This is the single pretty-table primitive for rank subcommands that color
/// rows by severity (complexity, length, test-gaps). Pass `|_| None` for a
/// plain bold-title-only table.
pub fn pretty_ranked_table<E: RankEntry>(
    title: &str,
    entries: &[E],
    empty_message: Option<&str>,
    row_color: impl Fn(&E) -> Option<Color>,
) -> String {
    let table = format_ranked_table(title, entries, empty_message);
    let lines: Vec<&str> = table.lines().collect();
    // Layout from format_ranked_table: title, blank, [header, separator, rows...]
    // or title, blank, empty_message.
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut data_row_idx = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push(Style::new().bold().paint(*line).to_string());
        } else if i >= 4 && !entries.is_empty() {
            // Data rows start after title, blank, header, separator.
            match entries.get(data_row_idx).and_then(&row_color) {
                Some(color) => out.push(color.paint(*line).to_string()),
                None => out.push((*line).to_string()),
            }
            data_row_idx += 1;
        } else {
            out.push((*line).to_string());
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time verification that report types implement OutputFormatter.
    /// If a type listed here doesn't implement the trait, this will fail to compile.
    #[allow(dead_code)]
    fn assert_output_formatter<T: OutputFormatter>() {}

    #[test]
    fn test_output_formatter_implementations() {
        // Verify all structured report types implement OutputFormatter.
        // Add new report types here when they're created.
        // This is a compile-time check - if any type doesn't implement
        // OutputFormatter, the code won't compile.
        use crate::analyze::complexity::ComplexityReport;
        use crate::analyze::function_length::LengthReport;
        use crate::analyze::test_gaps::TestGapsReport;
        use crate::commands::aliases::AliasesReport;
        use crate::commands::analyze::activity::ActivityReport;
        use crate::commands::analyze::architecture::ArchitectureReport;
        use crate::commands::analyze::budget::LineBudgetReport;
        use crate::commands::analyze::call_complexity::CallComplexityReport;
        use crate::commands::analyze::ceremony::CeremonyReport;
        use crate::commands::analyze::contributors::ContributorsReport;
        use crate::commands::analyze::coupling::CouplingReport;
        use crate::commands::analyze::coupling_clusters::CouplingClustersReport;
        use crate::commands::analyze::cross_repo_health::CrossRepoHealthReport;
        use crate::commands::analyze::density::DensityReport;
        use crate::commands::analyze::depth_map::DepthMapReport;
        use crate::commands::analyze::docs::DocCoverageReport;
        use crate::commands::analyze::duplicates::DuplicateTypesReport;
        use crate::commands::analyze::duplicates_views::DuplicatesReport;
        use crate::commands::analyze::effects::EffectsReport;
        use crate::commands::analyze::exceptions::ExceptionsReport;
        use crate::commands::analyze::files::FileLengthReport;
        use crate::commands::analyze::fragments::FragmentsReport;
        use crate::commands::analyze::graph::{DependentsReport, GraphReport};
        use crate::commands::analyze::hotspots::HotspotsReport;
        use crate::commands::analyze::imports::ImportCentralityReport;
        use crate::commands::analyze::layering::LayeringReport;
        use crate::commands::analyze::liveness::LivenessReport;
        use crate::commands::analyze::module_health::ModuleHealthReport;
        use crate::commands::analyze::ownership::OwnershipReport;
        use crate::commands::analyze::provenance::ProvenanceReport;
        use crate::commands::analyze::repo_coupling::RepoCouplingReport;
        use crate::commands::analyze::report::{AnalyzeReport, SecurityReport};
        use crate::commands::analyze::size::SizeReport;
        use crate::commands::analyze::skeleton_diff::SkeletonDiffReport;
        use crate::commands::analyze::summary::SummaryReport;
        use crate::commands::analyze::surface::SurfaceReport;
        use crate::commands::analyze::test_ratio::TestRatioReport;
        use crate::commands::analyze::trend::TrendReport;
        use crate::commands::analyze::uniqueness::UniquenessReport;
        use crate::commands::ci::CiReport;
        use crate::commands::context::{ContextListReport, ContextReport};
        use crate::commands::find_references::ReferencesReport;
        use crate::commands::grammars::{GrammarListReport, GrammarPathsReport};
        use crate::commands::history::{
            HistoryDiffReport, HistoryListReport, HistoryPruneReport, HistoryStatusReport,
            HistoryTreeReport,
        };
        use crate::commands::tools::lint::LintListReport;
        use crate::commands::view::report::{ViewHistoryReport, ViewListReport, ViewReport};
        use crate::text_search::GrepReport;
        use normalize_output::diagnostics::DiagnosticsReport;
        use normalize_session_analysis::SessionAnalysisReport;
        use normalize_sessions::SessionShowReport;
        use normalize_sessions::SubagentsReport;
        use normalize_sessions::list::SessionListReport;
        use normalize_sessions::messages::MessagesReport;
        use normalize_sessions::plans::PlansListReport;

        // Compile-time checks via trait bounds
        assert_output_formatter::<CiReport>();
        assert_output_formatter::<ActivityReport>();
        assert_output_formatter::<CallComplexityReport>();
        assert_output_formatter::<DensityReport>();
        assert_output_formatter::<DepthMapReport>();
        assert_output_formatter::<UniquenessReport>();
        assert_output_formatter::<AnalyzeReport>();
        assert_output_formatter::<AliasesReport>();
        assert_output_formatter::<ArchitectureReport>();
        assert_output_formatter::<LineBudgetReport>();
        assert_output_formatter::<CeremonyReport>();
        assert_output_formatter::<DiagnosticsReport>();
        assert_output_formatter::<ComplexityReport>();
        assert_output_formatter::<ContributorsReport>();
        assert_output_formatter::<ContextListReport>();
        assert_output_formatter::<ContextReport>();
        assert_output_formatter::<CouplingClustersReport>();
        assert_output_formatter::<CouplingReport>();
        assert_output_formatter::<CrossRepoHealthReport>();
        assert_output_formatter::<DocCoverageReport>();
        assert_output_formatter::<EffectsReport>();
        assert_output_formatter::<ExceptionsReport>();
        assert_output_formatter::<DuplicateTypesReport>();
        assert_output_formatter::<DuplicatesReport>();
        assert_output_formatter::<FileLengthReport>();
        assert_output_formatter::<FragmentsReport>();
        assert_output_formatter::<ReferencesReport>();
        assert_output_formatter::<GrammarListReport>();
        assert_output_formatter::<GrammarPathsReport>();
        assert_output_formatter::<DependentsReport>();
        assert_output_formatter::<GraphReport>();
        assert_output_formatter::<GrepReport>();
        assert_output_formatter::<HistoryDiffReport>();
        assert_output_formatter::<HistoryListReport>();
        assert_output_formatter::<HistoryPruneReport>();
        assert_output_formatter::<HistoryStatusReport>();
        assert_output_formatter::<HistoryTreeReport>();
        assert_output_formatter::<HotspotsReport>();
        assert_output_formatter::<ImportCentralityReport>();
        assert_output_formatter::<LengthReport>();
        assert_output_formatter::<ModuleHealthReport>();
        assert_output_formatter::<LintListReport>();
        assert_output_formatter::<OwnershipReport>();
        assert_output_formatter::<ProvenanceReport>();
        assert_output_formatter::<PlansListReport>();
        assert_output_formatter::<RepoCouplingReport>();
        assert_output_formatter::<SecurityReport>();
        assert_output_formatter::<MessagesReport>();
        assert_output_formatter::<SessionAnalysisReport>();
        assert_output_formatter::<SessionListReport>();
        assert_output_formatter::<SessionShowReport>();
        assert_output_formatter::<SubagentsReport>();
        use normalize_sessions::ngrams::NgramsReport;
        assert_output_formatter::<NgramsReport>();
        use normalize_sessions::patterns::PatternsReport;
        assert_output_formatter::<PatternsReport>();
        use normalize_sessions::stats::RepoStatsReport;
        assert_output_formatter::<RepoStatsReport>();
        use crate::commands::sync::SyncReport;
        assert_output_formatter::<SyncReport>();
        assert_output_formatter::<SkeletonDiffReport>();
        assert_output_formatter::<SizeReport>();
        assert_output_formatter::<SummaryReport>();
        assert_output_formatter::<SurfaceReport>();
        assert_output_formatter::<LayeringReport>();
        assert_output_formatter::<TestGapsReport>();
        assert_output_formatter::<TestRatioReport>();
        assert_output_formatter::<TrendReport>();
        assert_output_formatter::<ViewHistoryReport>();
        assert_output_formatter::<ViewReport>();
        assert_output_formatter::<ViewListReport>();
        assert_output_formatter::<crate::commands::view::chunked::ChunkedViewReport>();

        use crate::service::config::{
            ConfigSchemaReport, ConfigSetReport, ConfigShowReport, ConfigValidateReport,
        };
        assert_output_formatter::<ConfigSchemaReport>();
        assert_output_formatter::<ConfigShowReport>();
        assert_output_formatter::<ConfigValidateReport>();
        assert_output_formatter::<ConfigSetReport>();

        use crate::commands::syntax::node_types::NodeTypesReport;
        assert_output_formatter::<NodeTypesReport>();

        use normalize_ratchet::service::{
            AddReport, CheckReport, MeasureReport, RemoveReport, ShowReport, UpdateReport,
        };
        assert_output_formatter::<MeasureReport>();
        assert_output_formatter::<CheckReport>();
        assert_output_formatter::<UpdateReport>();
        assert_output_formatter::<ShowReport>();
        assert_output_formatter::<AddReport>();
        assert_output_formatter::<RemoveReport>();

        use normalize_budget::service::{
            AddReport as BudgetAddReport, CheckReport as BudgetCheckReport,
            MeasureReport as BudgetMeasureReport, RemoveReport as BudgetRemoveReport,
            ShowReport as BudgetShowReport, UpdateReport as BudgetUpdateReport,
        };
        assert_output_formatter::<BudgetMeasureReport>();
        assert_output_formatter::<BudgetCheckReport>();
        assert_output_formatter::<BudgetAddReport>();
        assert_output_formatter::<BudgetUpdateReport>();
        assert_output_formatter::<BudgetShowReport>();
        assert_output_formatter::<BudgetRemoveReport>();

        // Task 4: missing entries
        use crate::commands::analyze::trend::ScalarTrendReport;
        assert_output_formatter::<ScalarTrendReport>();

        use crate::health::HealthReport;
        assert_output_formatter::<HealthReport>();

        use normalize_rules::{
            RulesCompileReport, RulesFixtureTestReport, RulesListReport, RulesTestReport,
            RulesValidateReport,
        };
        assert_output_formatter::<RulesListReport>();
        assert_output_formatter::<RulesValidateReport>();
        assert_output_formatter::<RulesCompileReport>();
        assert_output_formatter::<RulesTestReport>();
        assert_output_formatter::<RulesFixtureTestReport>();

        use normalize_native_rules::check_examples::CheckExamplesReport;
        use normalize_native_rules::check_refs::CheckRefsReport;
        use normalize_native_rules::stale_docs::StaleDocsReport;
        use normalize_native_rules::stale_summary::{MissingSummaryReport, StaleSummaryReport};
        use normalize_native_rules::{BudgetRulesReport, RatchetRulesReport};
        assert_output_formatter::<BudgetRulesReport>();
        assert_output_formatter::<RatchetRulesReport>();
        assert_output_formatter::<CheckExamplesReport>();
        assert_output_formatter::<CheckRefsReport>();
        assert_output_formatter::<StaleDocsReport>();
        assert_output_formatter::<MissingSummaryReport>();
        assert_output_formatter::<StaleSummaryReport>();

        // Service report types now implementing OutputFormatter
        use crate::service::facts::{
            CommandReport, ExtractionFixtureTestReport, FactsStats, FactsStatsReport,
            FileListReport, PackagesReport, QueryReport, RebuildReport, StorageReport,
        };
        assert_output_formatter::<RebuildReport>();
        assert_output_formatter::<FactsStats>();
        assert_output_formatter::<StorageReport>();
        assert_output_formatter::<FileListReport>();
        assert_output_formatter::<PackagesReport>();
        assert_output_formatter::<CommandReport>();
        assert_output_formatter::<FactsStatsReport>();
        assert_output_formatter::<QueryReport>();
        assert_output_formatter::<ExtractionFixtureTestReport>();

        use crate::service::context::{ContextKindReport, ContextMigrateReport};
        assert_output_formatter::<ContextKindReport>();
        assert_output_formatter::<ContextMigrateReport>();

        use crate::service::docs::DocsReport;
        assert_output_formatter::<DocsReport>();
        use crate::service::{InitReport, TranslateReport};
        assert_output_formatter::<InitReport>();
        assert_output_formatter::<TranslateReport>();
        use crate::service::UpdateReport as ServiceUpdateReport;
        assert_output_formatter::<ServiceUpdateReport>();

        use crate::service::grammars::GrammarInstallReport;
        assert_output_formatter::<GrammarInstallReport>();

        use crate::service::generate::GenerateReport;
        assert_output_formatter::<GenerateReport>();

        use crate::service::daemon::{
            DaemonActionReport, DaemonRootReport, DaemonRootsReport, DaemonRunReport,
        };
        assert_output_formatter::<DaemonActionReport>();
        assert_output_formatter::<DaemonRunReport>();
        assert_output_formatter::<DaemonRootReport>();
        assert_output_formatter::<DaemonRootsReport>();

        use crate::commands::analyze::import_path::ImportPathReport;
        assert_output_formatter::<ImportPathReport>();

        use crate::service::view::TraceReport;
        assert_output_formatter::<TraceReport>();

        use crate::service::rename::RenameReport;
        assert_output_formatter::<RenameReport>();

        use crate::service::edit::{
            AddParameterReport, ExtractFunctionReport, InlineFunctionReport, InlineVariableReport,
            IntroduceVariableReport, MoveReport,
        };
        assert_output_formatter::<MoveReport>();
        assert_output_formatter::<IntroduceVariableReport>();
        assert_output_formatter::<InlineVariableReport>();
        assert_output_formatter::<AddParameterReport>();
        assert_output_formatter::<InlineFunctionReport>();
        assert_output_formatter::<ExtractFunctionReport>();

        use normalize_rules::{RuleInfoReport, RuleShowReport, RulesTagsReport};
        assert_output_formatter::<RuleShowReport>();
        assert_output_formatter::<RuleInfoReport>();
        assert_output_formatter::<RulesTagsReport>();

        use crate::commands::tools::test::{TestListReport, TestRunReport};
        assert_output_formatter::<TestListReport>();
        assert_output_formatter::<TestRunReport>();

        use crate::commands::tools::lint::LintRunReport;
        assert_output_formatter::<LintRunReport>();

        use normalize_knowledge_graph::reports::{
            ReadReport as KgReadReport, UnitReport as KgUnitReport, WalkReport as KgWalkReport,
            WriteReport as KgWriteReport,
        };
        assert_output_formatter::<KgUnitReport>();
        assert_output_formatter::<KgReadReport>();
        assert_output_formatter::<KgWriteReport>();
        assert_output_formatter::<KgWalkReport>();

        use normalize_sessions::service::PlansReport;
        assert_output_formatter::<PlansReport>();

        use normalize_sessions::mark::MarkReport;
        assert_output_formatter::<MarkReport>();

        use crate::service::package::{
            PackageAuditReport, PackageInfoReport, PackageListReport, PackageOutdatedReport,
            PackageTreeReport, PackageWhyReport,
        };
        assert_output_formatter::<PackageInfoReport>();
        assert_output_formatter::<PackageListReport>();
        assert_output_formatter::<PackageTreeReport>();
        assert_output_formatter::<PackageWhyReport>();
        assert_output_formatter::<PackageOutdatedReport>();
        assert_output_formatter::<PackageAuditReport>();
        assert_output_formatter::<LivenessReport>();
    }
}
