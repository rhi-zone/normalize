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
        use crate::commands::analyze::budget::BudgetReport;
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
        use crate::commands::context::{ContextListReport, ContextReport};
        use crate::commands::find_references::ReferencesReport;
        use crate::commands::grammars::{GrammarListReport, GrammarPathsReport};
        use crate::commands::history::{
            HistoryDiffReport, HistoryListReport, HistoryPruneReport, HistoryStatusReport,
            HistoryTreeReport,
        };
        use crate::commands::sessions::SessionShowReport;
        use crate::commands::sessions::list::SessionListReport;
        use crate::commands::sessions::messages::MessagesReport;
        use crate::commands::sessions::plans::PlansListReport;
        use crate::commands::tools::lint::LintListResult;
        use crate::commands::view::report::ViewOutput;
        use crate::sessions::SessionAnalysis;
        use crate::text_search::GrepResult;
        use normalize_output::diagnostics::DiagnosticsReport;

        // Compile-time checks via trait bounds
        assert_output_formatter::<ActivityReport>();
        assert_output_formatter::<CallComplexityReport>();
        assert_output_formatter::<DensityReport>();
        assert_output_formatter::<DepthMapReport>();
        assert_output_formatter::<UniquenessReport>();
        assert_output_formatter::<AnalyzeReport>();
        assert_output_formatter::<AliasesReport>();
        assert_output_formatter::<ArchitectureReport>();
        assert_output_formatter::<BudgetReport>();
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
        assert_output_formatter::<GrepResult>();
        assert_output_formatter::<HistoryDiffReport>();
        assert_output_formatter::<HistoryListReport>();
        assert_output_formatter::<HistoryPruneReport>();
        assert_output_formatter::<HistoryStatusReport>();
        assert_output_formatter::<HistoryTreeReport>();
        assert_output_formatter::<HotspotsReport>();
        assert_output_formatter::<ImportCentralityReport>();
        assert_output_formatter::<LengthReport>();
        assert_output_formatter::<ModuleHealthReport>();
        assert_output_formatter::<LintListResult>();
        assert_output_formatter::<OwnershipReport>();
        assert_output_formatter::<ProvenanceReport>();
        assert_output_formatter::<PlansListReport>();
        assert_output_formatter::<RepoCouplingReport>();
        assert_output_formatter::<SecurityReport>();
        assert_output_formatter::<MessagesReport>();
        assert_output_formatter::<SessionAnalysis>();
        assert_output_formatter::<SessionListReport>();
        assert_output_formatter::<SessionShowReport>();
        assert_output_formatter::<SkeletonDiffReport>();
        assert_output_formatter::<SizeReport>();
        assert_output_formatter::<SummaryReport>();
        assert_output_formatter::<SurfaceReport>();
        assert_output_formatter::<LayeringReport>();
        assert_output_formatter::<TestGapsReport>();
        assert_output_formatter::<TestRatioReport>();
        assert_output_formatter::<TrendReport>();
        assert_output_formatter::<ViewOutput>();

        use crate::service::config::{ConfigSetReport, ConfigShowReport, ConfigValidateReport};
        assert_output_formatter::<ConfigShowReport>();
        assert_output_formatter::<ConfigValidateReport>();
        assert_output_formatter::<ConfigSetReport>();

        use crate::commands::analyze::ts_node_types::NodeTypesReport;
        use crate::commands::analyze::ts_parse::ParseReport;
        use crate::commands::analyze::ts_query::QueryReport;
        assert_output_formatter::<NodeTypesReport>();
        assert_output_formatter::<ParseReport>();
        assert_output_formatter::<QueryReport>();
    }
}
