import * as vscode from 'vscode';
import * as path from 'path';

export class SkeletonViewProvider {
    showSkeleton(skeleton: string, filePath: string): void {
        const fileName = path.basename(filePath);

        const panel = vscode.window.createWebviewPanel(
            'normalizeSkeleton',
            `Skeleton: ${fileName}`,
            vscode.ViewColumn.Beside,
            {
                enableScripts: false,
                retainContextWhenHidden: true
            }
        );

        panel.webview.html = this.getWebviewContent(skeleton, fileName);
    }

    private getWebviewContent(skeleton: string, fileName: string): string {
        const escapedSkeleton = this.escapeHtml(skeleton);

        return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Skeleton: ${this.escapeHtml(fileName)}</title>
    <style>
        body {
            font-family: var(--vscode-editor-font-family, monospace);
            font-size: var(--vscode-editor-font-size, 14px);
            line-height: 1.5;
            padding: 16px;
            background-color: var(--vscode-editor-background);
            color: var(--vscode-editor-foreground);
        }
        pre {
            margin: 0;
            white-space: pre-wrap;
            word-wrap: break-word;
        }
        .header {
            font-weight: bold;
            margin-bottom: 16px;
            padding-bottom: 8px;
            border-bottom: 1px solid var(--vscode-panel-border);
        }
        .keyword {
            color: var(--vscode-symbolIcon-keywordForeground, #569cd6);
        }
        .function {
            color: var(--vscode-symbolIcon-functionForeground, #dcdcaa);
        }
        .class {
            color: var(--vscode-symbolIcon-classForeground, #4ec9b0);
        }
        .comment {
            color: var(--vscode-symbolIcon-commentForeground, #6a9955);
        }
    </style>
</head>
<body>
    <div class="header">Code Skeleton: ${this.escapeHtml(fileName)}</div>
    <pre>${escapedSkeleton}</pre>
</body>
</html>`;
    }

    private escapeHtml(text: string): string {
        return text
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#039;');
    }
}
