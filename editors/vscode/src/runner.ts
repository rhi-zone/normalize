import * as vscode from 'vscode';
import { spawn } from 'child_process';
import * as path from 'path';

export interface RulesResult {
    violations: Violation[];
    total_violations: number;
    files_analyzed: number;
}

export interface Violation {
    rule_id: string;
    message: string;
    file: string;
    line: number;
    column: number;
    severity: 'error' | 'warning' | 'info';
}

export class MossRunner {
    private pythonPath: string;

    constructor() {
        this.pythonPath = this.getPythonPath();
    }

    private getPythonPath(): string {
        const config = vscode.workspace.getConfiguration('moss');
        return config.get<string>('pythonPath') ?? 'python';
    }

    updatePythonPath(): void {
        this.pythonPath = this.getPythonPath();
    }

    private async runMoss(args: string[], cwd?: string): Promise<string> {
        return new Promise((resolve, reject) => {
            const process = spawn(this.pythonPath, ['-m', 'moss', ...args], {
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

    async runRules(targetPath: string): Promise<RulesResult> {
        const config = vscode.workspace.getConfiguration('moss');
        const args = ['rules', targetPath, '--json'];

        if (config.get<boolean>('builtinRules')) {
            args.push('--builtin');
        }

        const output = await this.runMoss(args, path.dirname(targetPath));

        try {
            return JSON.parse(output) as RulesResult;
        } catch {
            // If JSON parsing fails, return empty result
            return {
                violations: [],
                total_violations: 0,
                files_analyzed: 0
            };
        }
    }

    async runMetrics(targetPath: string): Promise<string> {
        const args = ['metrics', targetPath, '--html'];
        return await this.runMoss(args, targetPath);
    }

    async runSkeleton(filePath: string): Promise<string> {
        const args = ['skeleton', filePath];
        return await this.runMoss(args, path.dirname(filePath));
    }

    async runDiffAnalysis(workspacePath: string, ref: string): Promise<string> {
        const args = ['diff', ref, '--format', 'markdown'];
        return await this.runMoss(args, workspacePath);
    }
}
