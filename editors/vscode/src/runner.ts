import * as vscode from 'vscode';
import { spawn } from 'child_process';
import * as path from 'path';

export interface LintResult {
    tool: string;
    diagnostics: Diagnostic[];
    success: boolean;
    error?: string;
}

export interface Diagnostic {
    tool: string;
    rule_id: string;
    message: string;
    severity: 'error' | 'warning';
    location: {
        file: string;
        line: number;
        column: number;
        end_line?: number;
        end_column?: number;
    };
    fix?: {
        description: string;
        replacement: string;
    };
    help_url?: string;
}

export interface SarifResult {
    version: string;
    runs: SarifRun[];
}

export interface SarifRun {
    tool: {
        driver: {
            name: string;
        };
    };
    results: SarifDiagnostic[];
}

export interface SarifDiagnostic {
    ruleId: string;
    message: { text: string };
    level: 'error' | 'warning' | 'note';
    locations: {
        physicalLocation: {
            artifactLocation: { uri: string };
            region: {
                startLine: number;
                startColumn: number;
                endLine?: number;
                endColumn?: number;
            };
        };
    }[];
}

export class NormalizeRunner {
    private binaryPath: string;

    constructor() {
        this.binaryPath = this.getBinaryPath();
    }

    private getBinaryPath(): string {
        const config = vscode.workspace.getConfiguration('normalize');
        return config.get<string>('binaryPath') ?? 'normalize';
    }

    updateBinaryPath(): void {
        this.binaryPath = this.getBinaryPath();
    }

    private async runNormalize(args: string[], cwd?: string): Promise<string> {
        return new Promise((resolve, reject) => {
            const process = spawn(this.binaryPath, args, {
                cwd: cwd ?? vscode.workspace.workspaceFolders?.[0]?.uri.fsPath
            });

            let stdout = '';
            let stderr = '';

            process.stdout.on('data', (data) => {
                stdout += data.toString();
            });

            process.stderr.on('data', (data) => {
                stderr += data.toString();
            });

            process.on('close', (code) => {
                if (code === 0 || stdout) {
                    resolve(stdout);
                } else {
                    reject(new Error(stderr || `Process exited with code ${code}`));
                }
            });

            process.on('error', (error) => {
                reject(error);
            });
        });
    }

    async runLint(targetPath: string, fix: boolean = false): Promise<LintResult[]> {
        const config = vscode.workspace.getConfiguration('normalize');
        const categories = config.get<string[]>('lintCategories') ?? ['linter'];

        const args = ['lint', targetPath, '--sarif'];

        if (fix) {
            args.push('--fix');
        }

        for (const category of categories) {
            args.push('--category', category);
        }

        try {
            const output = await this.runNormalize(args, path.dirname(targetPath));
            const sarif = JSON.parse(output) as SarifResult;

            return sarif.runs.map((run) => ({
                tool: run.tool.driver.name,
                diagnostics: run.results.map((result) => {
                    const loc = result.locations[0]?.physicalLocation;
                    return {
                        tool: run.tool.driver.name,
                        rule_id: result.ruleId,
                        message: result.message.text,
                        severity: result.level === 'error' ? 'error' : 'warning',
                        location: {
                            file: loc?.artifactLocation.uri ?? '',
                            line: loc?.region.startLine ?? 1,
                            column: loc?.region.startColumn ?? 1,
                            end_line: loc?.region.endLine,
                            end_column: loc?.region.endColumn,
                        },
                    } as Diagnostic;
                }),
                success: true,
            }));
        } catch (error) {
            return [{
                tool: 'normalize',
                diagnostics: [],
                success: false,
                error: String(error),
            }];
        }
    }

    async runSkeleton(filePath: string): Promise<string> {
        const args = ['view', filePath];
        return await this.runNormalize(args, path.dirname(filePath));
    }

    async runViewTree(dirPath: string, depth: number = 2): Promise<string> {
        const args = ['view', dirPath, '--depth', String(depth)];
        return await this.runNormalize(args, dirPath);
    }

    async runAnalyzeHealth(targetPath: string): Promise<string> {
        const args = ['analyze', '--health', targetPath, '--json'];
        return await this.runNormalize(args, targetPath);
    }
}
