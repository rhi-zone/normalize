import * as vscode from 'vscode';
import { MossRunner } from './runner';
import { MossDiagnostics } from './diagnostics';
import { SkeletonViewProvider } from './skeleton';

let runner: MossRunner;
let diagnostics: MossDiagnostics;
let skeletonProvider: SkeletonViewProvider;

export function activate(context: vscode.ExtensionContext) {
    console.log('Moss extension activated');

    // Initialize components
    runner = new MossRunner();
    diagnostics = new MossDiagnostics();
    skeletonProvider = new SkeletonViewProvider();

    // Register diagnostics collection
    const diagnosticCollection = vscode.languages.createDiagnosticCollection('moss');
    context.subscriptions.push(diagnosticCollection);

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('moss.runRules', async (uri?: vscode.Uri) => {
            const targetPath = uri?.fsPath ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
            if (!targetPath) {
                vscode.window.showErrorMessage('No workspace folder open');
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Running Moss rules check...',
                    cancellable: false
                },
                async () => {
                    try {
                        const result = await runner.runRules(targetPath);
                        diagnostics.updateDiagnostics(diagnosticCollection, result);

                        const violations = result.violations?.length ?? 0;
                        if (violations === 0) {
                            vscode.window.showInformationMessage('Moss: No violations found');
                        } else {
                            vscode.window.showWarningMessage(
                                `Moss: Found ${violations} violation(s)`
                            );
                        }
                    } catch (error) {
                        vscode.window.showErrorMessage(`Moss error: ${error}`);
                    }
                }
            );
        }),

        vscode.commands.registerCommand('moss.showMetrics', async () => {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            if (!workspaceFolder) {
                vscode.window.showErrorMessage('No workspace folder open');
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Generating Moss metrics...',
                    cancellable: false
                },
                async () => {
                    try {
                        const html = await runner.runMetrics(workspaceFolder.uri.fsPath);
                        const panel = vscode.window.createWebviewPanel(
                            'mossMetrics',
                            'Moss Metrics Dashboard',
                            vscode.ViewColumn.One,
                            { enableScripts: true }
                        );
                        panel.webview.html = html;
                    } catch (error) {
                        vscode.window.showErrorMessage(`Moss error: ${error}`);
                    }
                }
            );
        }),

        vscode.commands.registerCommand('moss.showSkeleton', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                vscode.window.showErrorMessage('No active editor');
                return;
            }

            if (editor.document.languageId !== 'python') {
                vscode.window.showErrorMessage('Skeleton view only supports Python files');
                return;
            }

            try {
                const skeleton = await runner.runSkeleton(editor.document.uri.fsPath);
                skeletonProvider.showSkeleton(skeleton, editor.document.fileName);
            } catch (error) {
                vscode.window.showErrorMessage(`Moss error: ${error}`);
            }
        }),

        vscode.commands.registerCommand('moss.analyzeDiff', async () => {
            const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
            if (!workspaceFolder) {
                vscode.window.showErrorMessage('No workspace folder open');
                return;
            }

            const ref = await vscode.window.showInputBox({
                prompt: 'Enter git reference to compare against',
                value: 'HEAD~1',
                placeHolder: 'e.g., HEAD~1, main, abc123'
            });

            if (!ref) {
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'Analyzing git diff...',
                    cancellable: false
                },
                async () => {
                    try {
                        const analysis = await runner.runDiffAnalysis(
                            workspaceFolder.uri.fsPath,
                            ref
                        );

                        const doc = await vscode.workspace.openTextDocument({
                            content: analysis,
                            language: 'markdown'
                        });
                        await vscode.window.showTextDocument(doc);
                    } catch (error) {
                        vscode.window.showErrorMessage(`Moss error: ${error}`);
                    }
                }
            );
        })
    );

    // Set up on-save diagnostics if enabled
    const config = vscode.workspace.getConfiguration('moss');
    if (config.get<boolean>('runOnSave')) {
        context.subscriptions.push(
            vscode.workspace.onDidSaveTextDocument(async (document) => {
                if (document.languageId === 'python') {
                    try {
                        const result = await runner.runRules(document.uri.fsPath);
                        diagnostics.updateDiagnostics(diagnosticCollection, result);
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
            if (e.affectsConfiguration('moss.pythonPath')) {
                runner.updatePythonPath();
            }
        })
    );
}

export function deactivate() {
    console.log('Moss extension deactivated');
}
