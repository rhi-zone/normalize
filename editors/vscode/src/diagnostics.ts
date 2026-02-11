import * as vscode from 'vscode';
import { LintResult, Diagnostic } from './runner';

export class NormalizeDiagnostics {
    updateDiagnosticsFromLint(
        collection: vscode.DiagnosticCollection,
        results: LintResult[]
    ): void {
        // Clear existing diagnostics
        collection.clear();

        // Flatten all diagnostics from all tools
        const allDiagnostics: Diagnostic[] = [];
        for (const result of results) {
            if (result.success) {
                allDiagnostics.push(...result.diagnostics);
            }
        }

        // Group by file
        const byFile = new Map<string, Diagnostic[]>();
        for (const diag of allDiagnostics) {
            const file = diag.location.file;
            const existing = byFile.get(file) ?? [];
            existing.push(diag);
            byFile.set(file, existing);
        }

        // Create VS Code diagnostics for each file
        for (const [filePath, diagnostics] of byFile) {
            const uri = vscode.Uri.file(filePath);
            const vscodeDiagnostics = diagnostics.map((d) => this.toDiagnostic(d));
            collection.set(uri, vscodeDiagnostics);
        }
    }

    private toDiagnostic(diag: Diagnostic): vscode.Diagnostic {
        const startLine = Math.max(0, diag.location.line - 1);
        const startColumn = Math.max(0, diag.location.column - 1);
        const endLine = diag.location.end_line ? Math.max(0, diag.location.end_line - 1) : startLine;
        const endColumn = diag.location.end_column ? Math.max(0, diag.location.end_column - 1) : startColumn + 100;

        const range = new vscode.Range(
            new vscode.Position(startLine, startColumn),
            new vscode.Position(endLine, endColumn)
        );

        const severity = diag.severity === 'error'
            ? vscode.DiagnosticSeverity.Error
            : vscode.DiagnosticSeverity.Warning;

        const diagnostic = new vscode.Diagnostic(
            range,
            diag.message,
            severity
        );

        diagnostic.source = diag.tool;
        diagnostic.code = diag.rule_id;

        if (diag.help_url) {
            diagnostic.code = {
                value: diag.rule_id,
                target: vscode.Uri.parse(diag.help_url)
            };
        }

        return diagnostic;
    }
}
