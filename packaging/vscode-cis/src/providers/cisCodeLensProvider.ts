import * as vscode from 'vscode';

/**
 * CodeLens 提供器 - 在 DAG 文件中显示执行按钮
 */
export class CisCodeLensProvider implements vscode.CodeLensProvider {
    private _onDidChangeCodeLenses: vscode.EventEmitter<void> = new vscode.EventEmitter<void>();
    public readonly onDidChangeCodeLenses: vscode.Event<void> = this._onDidChangeCodeLenses.event;

    provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
        const codeLenses: vscode.CodeLens[] = [];
        const text = document.getText();

        // 检查是否是 DAG 配置文件
        if (!this.isDagFile(text)) {
            return codeLenses;
        }

        // 设置上下文变量
        vscode.commands.executeCommand('setContext', 'cis.isDagFile', true);

        // 查找 [dag] 或 [pipeline] 部分
        const dagSection = this.findSection(document, 'dag') || this.findSection(document, 'pipeline');
        if (dagSection) {
            // 提取 DAG 名称
            const dagName = this.extractDagName(text) || document.fileName.split('/').pop()?.replace('.dag.toml', '') || 'unknown';

            // 在 DAG 部分添加 "运行" 按钮
            const runCommand: vscode.Command = {
                title: '▶ 运行 DAG',
                command: 'cis.runDag',
                arguments: [{ dag: { id: dagName } }]
            };
            codeLenses.push(new vscode.CodeLens(dagSection, runCommand));

            // 添加 "运行（带参数）" 按钮
            const runWithArgsCommand: vscode.Command = {
                title: '⚙ 运行（带参数）',
                command: 'cis.runDagWithArgs',
                arguments: [{ dag: { id: dagName } }]
            };
            codeLenses.push(new vscode.CodeLens(dagSection, runWithArgsCommand));
        }

        // 查找所有 [step.*] 部分
        const stepPattern = /^\[step\.(\w+)\]/gm;
        let match;
        while ((match = stepPattern.exec(text)) !== null) {
            const stepName = match[1];
            const position = document.positionAt(match.index);
            const range = new vscode.Range(position, position.translate(0, match[0].length));

            const stepCommand: vscode.Command = {
                title: `⏵ 运行到 ${stepName}`,
                command: 'cis.runDag',
                arguments: [{ dag: { id: this.extractDagName(text) || 'unknown' }, stopAt: stepName }]
            };
            codeLenses.push(new vscode.CodeLens(range, stepCommand));
        }

        return codeLenses;
    }

    /**
     * 检查文件是否是 DAG 配置文件
     */
    private isDagFile(text: string): boolean {
        // 检查是否包含 [dag] 或 [pipeline] 部分
        return /^\[(dag|pipeline)\]/m.test(text);
    }

    /**
     * 查找配置部分的位置
     */
    private findSection(document: vscode.TextDocument, sectionName: string): vscode.Range | undefined {
        const text = document.getText();
        const pattern = new RegExp(`^\\[${sectionName}\\]`, 'm');
        const match = pattern.exec(text);
        
        if (match) {
            const position = document.positionAt(match.index);
            return new vscode.Range(position, position.translate(0, match[0].length));
        }
        return undefined;
    }

    /**
     * 从文本中提取 DAG 名称
     */
    private extractDagName(text: string): string | undefined {
        // 查找 name = "xxx" 或 name = 'xxx'
        const nameMatch = text.match(/^name\s*=\s*["'](.+?)["']/m);
        if (nameMatch) {
            return nameMatch[1];
        }
        return undefined;
    }
}
