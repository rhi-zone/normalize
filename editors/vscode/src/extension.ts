import * as vscode from 'vscode';
import { NormalizeRunner } from './runner';
import { NormalizeDiagnostics } from './diagnostics';
import { SkeletonViewProvider } from './skeleton';

let runner: NormalizeRunner;
let diagnostics: NormalizeDiagnostics;
let skeletonProvider: SkeletonViewProvider;

export function activate(context: vscode.ExtensionContext) {
    console.log('Normalize extension activated');

    // Initialize components
    runner = new NormalizeRunner();
    diagnostics = new NormalizeDiagnostics();
    skeletonProvider = new SkeletonViewProvider();

    // Register diagnostics collection
    const diagnosticCollection = vscode.languages.createDiagnosticCollection('normalize');
    context.subscriptions.push(diagnosticCollection);

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('normalize.lint', async (uri?: vscode.Uri) => {
            const targetPath = uri?.fsPath ?? vscode.window.activeTextEditor?.document.uri.fsPath ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            if (!targetPath) {
                vscode.window.showErrorMessage('No file or workspace folder open');
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Running Normalize lint...',
                    cancellable: false
                },
                async () => {
                    try {
                        const results = await runner.runLint(targetPath);
                        diagnostics.updateDiagnosticsFromLint(diagnosticCollection, results);

                        const totalDiagnostics = results.reduce((sum, r) => sum + r.diagnostics.length, 0);
                        const tools = results.filter(r => r.success).map(r => r.tool).join(', ');

                        if (totalDiagnostics === 0) {
                            vscode.window.showInformationMessage(`Normalize: No issues found (${tools})`);
                        } else {
                            vscode.window.showWarningMessage(
                                `Normalize: Found ${totalDiagnostics} issue(s) from ${tools}`
                            );
                        }
                    } catch (error) {
                        vscode.window.showErrorMessage(`Normalize error: ${error}`);
                    }
                }
            );
        }),

        vscode.commands.registerCommand('normalize.lintFix', async (uri?: vscode.Uri) => {
            const targetPath = uri?.fsPath ?? vscode.window.activeTextEditor?.document.uri.fsPath ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            if (!targetPath) {
                vscode.window.showErrorMessage('No file or workspace folder open');
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Running Normalize lint with auto-fix...',
                    cancellable: false
                },
                async () => {
                    try {
                        const results = await runner.runLint(targetPath, true);
                        diagnostics.updateDiagnosticsFromLint(diagnosticCollection, results);

                        const remainingDiagnostics = results.reduce((sum, r) => sum + r.diagnostics.length, 0);
                        if (remainingDiagnostics === 0) {
                            vscode.window.showInformationMessage('Normalize: All issues fixed');
                        } else {
                            vscode.window.showWarningMessage(
                                `Normalize: ${remainingDiagnostics} unfixable issue(s) remaining`
                            );
                        }
                    } catch (error) {
                        vscode.window.showErrorMessage(`Normalize error: ${error}`);
                    }
                }
            );
        }),

        vscode.commands.registerCommand('normalize.showSkeleton', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active editor');
                return;
            }

            try {
                const skeleton = await runner.runSkeleton(editor.document.uri.fsPath);
                skeletonProvider.showSkeleton(skeleton, editor.document.fileName);
            } catch (error) {
                vscode.window.showErrorMessage(`Normalize error: ${error}`);
            }
        }),

        vscode.commands.registerCommand('normalize.viewTree', async (uri?: vscode.Uri) => {
            const targetPath = uri?.fsPath ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            if (!targetPath) {
                vscode.window.showErrorMessage('No workspace folder open');
                return;
            }

            try {
                const tree = await runner.runViewTree(targetPath);
                const doc = await vscode.workspace.openTextDocument({
                    content: tree,
                    language: 'plaintext'
                });
                await vscode.window.showTextDocument(doc);
            } catch (error) {
                vscode.window.showErrorMessage(`Normalize error: ${error}`);
            }
        }),

        vscode.commands.registerCommand('normalize.analyzeHealth', async () => {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            if (!workspaceFolder) {
                vscode.window.showErrorMessage('No workspace folder open');
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Analyzing codebase health...',
                    cancellable: false
                },
                async () => {
                    try {
                        const health = await runner.runAnalyzeHealth(workspaceFolder.uri.fsPath);
                        const doc = await vscode.workspace.openTextDocument({
                            content: health,
                            language: 'json'
                        });
                        await vscode.window.showTextDocument(doc);
                    } catch (error) {
                        vscode.window.showErrorMessage(`Normalize error: ${error}`);
                    }
                }
            );
        })
    );

    // Set up on-save diagnostics if enabled
    const config = vscode.workspace.getConfiguration('normalize');
    if (config.get<boolean>('runOnSave')) {
        context.subscriptions.push(
            vscode.workspace.onDidSaveTextDocument(async (document) => {
                const supportedLanguages = ['python', 'typescript', 'typescriptreact', 'javascript', 'javascriptreact', 'rust', 'go'];
                if (supportedLanguages.includes(document.languageId)) {
                    try {
                        const results = await runner.runLint(document.uri.fsPath);
                        diagnostics.updateDiagnosticsFromLint(diagnosticCollection, results);
                    } catch {
                        // Silently ignore errors on auto-run
                    }
                }
            })
        );
    }

    // Watch for configuration changes
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration('normalize.binaryPath')) {
                runner.updateBinaryPath();
            }
        })
    );
}

export function deactivate() {
    console.log('Normalize extension deactivated');
}
