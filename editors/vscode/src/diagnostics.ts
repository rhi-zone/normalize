import * as vscode from 'vscode';
import { RulesResult, Violation } from './runner';

export class MossDiagnostics {
    updateDiagnostics(
        collection: vscode.DiagnosticCollection,
        result: RulesResult
    ): void {
        // Clear existing diagnostics
        collection.clear();

        // Group violations by file
        const byFile = new Map<string, Violation[]>();
        for (const violation of result.violations) {
            const existing = byFile.get(violation.file) ?? [];
            existing.push(violation);
            byFile.set(violation.file, existing);
        }

        // Create diagnostics for each file
        for (const [filePath, violations] of byFile) {
            const uri = vscode.Uri.file(filePath);
            const diagnostics = violations.map((v) => this.toDiagnostic(v));
            collection.set(uri, diagnostics);
        }
    }

    private toDiagnostic(violation: Violation): vscode.Diagnostic {
        const line = Math.max(0, violation.line - 1); // Convert to 0-based
        const column = Math.max(0, violation.column - 1);

        const range = new vscode.Range(
            new vscode.Position(line, column),
            new vscode.Position(line, column + 100) // Highlight to end of line
        );

        const severity = this.toSeverity(violation.severity);

        const diagnostic = new vscode.Diagnostic(
            range,
            violation.message,
            severity
        );

        diagnostic.source = 'moss';
        diagnostic.code = violation.rule_id;

        return diagnostic;
    }

    private toSeverity(severity: string): vscode.DiagnosticSeverity {
        switch (severity) {
            case 'error':
                return vscode.DiagnosticSeverity.Error;
            case 'warning':
                return vscode.DiagnosticSeverity.Warning;
            case 'info':
                return vscode.DiagnosticSeverity.Information;
            default:
                return vscode.DiagnosticSeverity.Warning;
        }
    }
}
