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
        use crate::commands::analyze::contributors::ContributorsReport;
        use crate::commands::analyze::report::AnalyzeReport;
        use crate::commands::grammars::{GrammarListReport, GrammarPathsReport};
        use crate::commands::sessions::SessionShowReport;
        use crate::commands::sessions::list::SessionListReport;
        use crate::commands::sessions::plans::PlansListReport;
        use crate::commands::tools::lint::LintListResult;
        use crate::commands::view::report::ViewOutput;
        use crate::sessions::SessionAnalysis;
        use crate::text_search::GrepResult;

        // Compile-time checks via trait bounds
        assert_output_formatter::<ComplexityReport>();
        assert_output_formatter::<LengthReport>();
        assert_output_formatter::<TestGapsReport>();
        assert_output_formatter::<AliasesReport>();
        assert_output_formatter::<AnalyzeReport>();
        assert_output_formatter::<GrammarListReport>();
        assert_output_formatter::<GrammarPathsReport>();
        assert_output_formatter::<SessionShowReport>();
        assert_output_formatter::<SessionListReport>();
        assert_output_formatter::<PlansListReport>();
        assert_output_formatter::<SessionAnalysis>();
        assert_output_formatter::<GrepResult>();
        assert_output_formatter::<LintListResult>();
        assert_output_formatter::<ViewOutput>();
        assert_output_formatter::<ContributorsReport>();
    }
}
