/// Compile-time verification that report types implement OutputFormatter.
/// If a type listed here doesn't implement the trait, this will fail to compile.
///
/// Moved from `crates/normalize/src/output.rs` #[cfg(test)] to an integration
/// test so it compiles as a separate crate — eliminating the intra-crate back-edges
/// that formed the main-crate module SCC.
#[allow(dead_code)]
fn assert_output_formatter<T: normalize_output::OutputFormatter>() {}

#[test]
fn test_output_formatter_implementations() {
    // Verify all structured report types implement OutputFormatter.
    // Add new report types here when they're created.
    // This is a compile-time check - if any type doesn't implement
    // OutputFormatter, the code won't compile.
    use normalize::analyze::complexity::ComplexityReport;
    use normalize::analyze::function_length::LengthReport;
    use normalize::analyze::test_gaps::TestGapsReport;
    use normalize::commands::analyze::budget::LineBudgetReport;
    use normalize::commands::analyze::call_complexity::CallComplexityReport;
    use normalize::commands::analyze::ceremony::CeremonyReport;
    use normalize::commands::analyze::coupling::CouplingReport;
    use normalize::commands::analyze::cross_repo_health::CrossRepoHealthReport;
    use normalize::commands::analyze::density::DensityReport;
    use normalize::commands::analyze::docs::DocCoverageReport;
    use normalize::commands::analyze::files::FileLengthReport;
    use normalize::commands::analyze::hotspots::HotspotsReport;
    use normalize::commands::analyze::imports::ImportCentralityReport;
    use normalize::commands::analyze::module_health::ModuleHealthReport;
    use normalize::commands::analyze::provenance::ProvenanceReport;
    use normalize::commands::analyze::report::{AnalyzeReport, SecurityReport};
    use normalize::commands::analyze::size::SizeReport;
    use normalize::commands::analyze::skeleton_diff::SkeletonDiffReport;
    use normalize::commands::analyze::summary::SummaryReport;
    use normalize::commands::analyze::surface::SurfaceReport;
    use normalize::commands::analyze::test_ratio::TestRatioReport;
    use normalize::commands::analyze::trend::TrendReport;
    use normalize::commands::analyze::uniqueness::UniquenessReport;
    use normalize::commands::ci::CiReport;
    use normalize::commands::context::{ContextListReport, ContextReport};
    use normalize::commands::find_references::ReferencesReport;
    use normalize::commands::grammars::{GrammarListReport, GrammarPathsReport};
    use normalize::commands::history::{
        HistoryDiffReport, HistoryListReport, HistoryPruneReport, HistoryStatusReport,
        HistoryTreeReport,
    };
    use normalize::commands::tools::lint::LintListReport;
    use normalize::commands::view::report::{ViewHistoryReport, ViewListReport, ViewReport};
    use normalize::text_search::GrepReport;
    use normalize_architecture::{ArchitectureReport, DepthMapReport, LayeringReport};
    use normalize_code_similarity::{DuplicateTypesReport, DuplicatesReport, FragmentsReport};
    use normalize_facts::service::{EffectsReport, ExceptionsReport, LivenessReport};
    use normalize_graph::{DependentsReport, GraphReport, ImportPathReport};
    use normalize_output::diagnostics::DiagnosticsReport;
    use normalize_semantic::service::SearchReport;
    use normalize_session_analysis::SessionAnalysisReport;
    use normalize_sessions::SessionShowReport;
    use normalize_sessions::SubagentsReport;
    use normalize_sessions::list::SessionListReport;
    use normalize_sessions::messages::MessagesReport;
    use normalize_sessions::plans::PlansListReport;

    // Compile-time checks via trait bounds
    assert_output_formatter::<CiReport>();
    assert_output_formatter::<CallComplexityReport>();
    assert_output_formatter::<DensityReport>();
    assert_output_formatter::<DepthMapReport>();
    assert_output_formatter::<UniquenessReport>();
    assert_output_formatter::<AnalyzeReport>();
    assert_output_formatter::<ArchitectureReport>();
    assert_output_formatter::<LineBudgetReport>();
    assert_output_formatter::<CeremonyReport>();
    assert_output_formatter::<DiagnosticsReport>();
    assert_output_formatter::<ComplexityReport>();
    assert_output_formatter::<ContextListReport>();
    assert_output_formatter::<ContextReport>();
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
    assert_output_formatter::<SearchReport>();
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
    assert_output_formatter::<ProvenanceReport>();
    assert_output_formatter::<PlansListReport>();
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
    use normalize::commands::sync::SyncReport;
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
    assert_output_formatter::<normalize::commands::view::chunked::ChunkedViewReport>();

    use normalize::service::config::{
        ConfigSchemaReport, ConfigSetReport, ConfigShowReport, ConfigValidateReport,
    };
    assert_output_formatter::<ConfigSchemaReport>();
    assert_output_formatter::<ConfigShowReport>();
    assert_output_formatter::<ConfigValidateReport>();
    assert_output_formatter::<ConfigSetReport>();

    use normalize::commands::syntax::node_types::NodeTypesReport;
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

    use normalize::commands::analyze::trend::ScalarTrendReport;
    assert_output_formatter::<ScalarTrendReport>();

    use normalize::health::HealthReport;
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
    use normalize_native_rules::{BudgetRulesReport, RatchetRulesReport};
    assert_output_formatter::<BudgetRulesReport>();
    assert_output_formatter::<RatchetRulesReport>();
    assert_output_formatter::<CheckExamplesReport>();
    assert_output_formatter::<CheckRefsReport>();
    assert_output_formatter::<StaleDocsReport>();

    // Service report types now implementing OutputFormatter
    use normalize_facts::service::{
        ExtractionFixtureTestReport, FactsStats, FactsStatsReport, FileListReport, PackagesReport,
        QueryReport, RebuildReport, StorageReport,
    };
    assert_output_formatter::<RebuildReport>();
    assert_output_formatter::<FactsStats>();
    assert_output_formatter::<StorageReport>();
    assert_output_formatter::<FileListReport>();
    assert_output_formatter::<PackagesReport>();
    assert_output_formatter::<FactsStatsReport>();
    assert_output_formatter::<QueryReport>();
    assert_output_formatter::<ExtractionFixtureTestReport>();

    use normalize::service::context::{ContextKindReport, ContextMigrateReport};
    assert_output_formatter::<ContextKindReport>();
    assert_output_formatter::<ContextMigrateReport>();

    use normalize::service::docs::DocsReport;
    assert_output_formatter::<DocsReport>();
    use normalize::service::{InitReport, TranslateReport};
    assert_output_formatter::<InitReport>();
    assert_output_formatter::<TranslateReport>();
    use normalize::service::UpdateReport as ServiceUpdateReport;
    assert_output_formatter::<ServiceUpdateReport>();

    use normalize::service::grammars::GrammarInstallReport;
    assert_output_formatter::<GrammarInstallReport>();

    use normalize::service::generate::GenerateReport;
    assert_output_formatter::<GenerateReport>();

    use normalize::service::daemon::{
        DaemonActionReport, DaemonRootReport, DaemonRootsReport, DaemonRunReport,
    };
    assert_output_formatter::<DaemonActionReport>();
    assert_output_formatter::<DaemonRunReport>();
    assert_output_formatter::<DaemonRootReport>();
    assert_output_formatter::<DaemonRootsReport>();

    assert_output_formatter::<ImportPathReport>();

    use normalize::service::view::TraceReport;
    assert_output_formatter::<TraceReport>();

    use normalize::service::rename::RenameReport;
    assert_output_formatter::<RenameReport>();

    use normalize::service::edit::{
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

    use normalize::commands::tools::test::{TestListReport, TestRunReport};
    assert_output_formatter::<TestListReport>();
    assert_output_formatter::<TestRunReport>();

    use normalize::commands::tools::lint::LintRunReport;
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

    use normalize::service::package::{
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
