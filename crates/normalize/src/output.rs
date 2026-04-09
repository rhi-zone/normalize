//! Output formatting utilities (re-exported from normalize-output).

pub use normalize_output::*;

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
        use crate::commands::analyze::files::FileLengthReport;
        use crate::commands::analyze::fragments::FragmentsReport;
        use crate::commands::analyze::graph::{DependentsReport, GraphReport};
        use crate::commands::analyze::hotspots::HotspotsReport;
        use crate::commands::analyze::imports::ImportCentralityReport;
        use crate::commands::analyze::layering::LayeringReport;
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
        use crate::commands::sessions::SessionShowReport;
        use crate::commands::sessions::SubagentsReport;
        use crate::commands::sessions::list::SessionListReport;
        use crate::commands::sessions::messages::MessagesReport;
        use crate::commands::sessions::plans::PlansListReport;
        use crate::commands::tools::lint::LintListReport;
        use crate::commands::view::report::{ViewHistoryReport, ViewListReport, ViewReport};
        use crate::sessions::SessionAnalysisReport;
        use crate::text_search::GrepReport;
        use normalize_output::diagnostics::DiagnosticsReport;

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
        use crate::commands::sessions::patterns::PatternsReport;
        assert_output_formatter::<PatternsReport>();
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

        use normalize_rules::{RulesCompileReport, RulesListReport, RulesValidateReport};
        assert_output_formatter::<RulesListReport>();
        assert_output_formatter::<RulesValidateReport>();
        assert_output_formatter::<RulesCompileReport>();

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
            CommandReport, FactsStats, FactsStatsReport, FileListReport, PackagesReport,
            QueryReport, RebuildReport, StorageReport,
        };
        assert_output_formatter::<RebuildReport>();
        assert_output_formatter::<FactsStats>();
        assert_output_formatter::<StorageReport>();
        assert_output_formatter::<FileListReport>();
        assert_output_formatter::<PackagesReport>();
        assert_output_formatter::<CommandReport>();
        assert_output_formatter::<FactsStatsReport>();
        assert_output_formatter::<QueryReport>();

        use crate::service::{ContextKindReport, InitReport, TranslateReport};
        assert_output_formatter::<ContextKindReport>();
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

        use crate::service::view::TraceReport;
        assert_output_formatter::<TraceReport>();

        use crate::service::rename::RenameReport;
        assert_output_formatter::<RenameReport>();

        use normalize_rules::{RuleInfoReport, RuleShowReport, RulesTagsReport};
        assert_output_formatter::<RuleShowReport>();
        assert_output_formatter::<RuleInfoReport>();
        assert_output_formatter::<RulesTagsReport>();

        use crate::commands::tools::test::{TestListReport, TestRunReport};
        assert_output_formatter::<TestListReport>();
        assert_output_formatter::<TestRunReport>();

        use crate::commands::tools::lint::LintRunReport;
        assert_output_formatter::<LintRunReport>();

        use crate::service::sessions::PlansReport;
        assert_output_formatter::<PlansReport>();

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

        use normalize_semantic::service::SearchReport;
        assert_output_formatter::<SearchReport>();
    }
}
