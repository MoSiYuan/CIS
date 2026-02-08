import * as vscode from 'vscode';
import { CisApiClient } from '../utils/cisApi';
import { CisNodeProvider } from '../providers/cisNodeProvider';
import { CisDagProvider, DagTreeItem } from '../providers/cisDagProvider';
import { CisTaskProvider, TaskTreeItem } from '../providers/cisTaskProvider';
import { CisDebtProvider } from '../providers/cisDebtProvider';

interface Providers {
    nodeProvider: CisNodeProvider;
    dagProvider: CisDagProvider;
    taskProvider: CisTaskProvider;
    debtProvider: CisDebtProvider;
}

export function registerCommands(
    context: vscode.ExtensionContext,
    cisApi: CisApiClient,
    providers: Providers,
    outputChannel: vscode.OutputChannel
): void {
    
    // 连接/断开命令
    const connectCommand = vscode.commands.registerCommand('cis.connect', async () => {
        const config = vscode.workspace.getConfiguration('cis');
        const nodeAddress = config.get<string>('nodeAddress') || 'http://localhost:7676';
        
        outputChannel.appendLine(`正在连接到 ${nodeAddress}...`);
        
        const connected = await cisApi.connect();
        if (connected) {
            vscode.window.showInformationMessage('已连接到 CIS 节点');
            providers.nodeProvider.refresh();
            providers.dagProvider.refresh();
            providers.taskProvider.refresh();
        } else {
            const result = await vscode.window.showErrorMessage(
                '连接 CIS 节点失败',
                '打开设置',
                '重试'
            );
            if (result === '打开设置') {
                vscode.commands.executeCommand('cis.openSettings');
            } else if (result === '重试') {
                vscode.commands.executeCommand('cis.connect');
            }
        }
    });

    const disconnectCommand = vscode.commands.registerCommand('cis.disconnect', () => {
        cisApi.disconnect();
        vscode.window.showInformationMessage('已断开与 CIS 节点的连接');
        providers.nodeProvider.refresh();
    });

    // 刷新命令
    const refreshCommand = vscode.commands.registerCommand('cis.refresh', () => {
        providers.nodeProvider.refresh();
        providers.dagProvider.refresh();
        providers.taskProvider.refresh();
        providers.debtProvider.refresh();
        vscode.window.showInformationMessage('已刷新');
    });

    // DAG 相关命令
    const runDagCommand = vscode.commands.registerCommand('cis.runDag', async (item: DagTreeItem | { dag: { id: string } }) => {
        const dagId = item instanceof DagTreeItem ? item.dag.id : item.dag.id;
        const taskId = await cisApi.runDag(dagId);
        
        if (taskId) {
            providers.taskProvider.refresh();
            
            const config = vscode.workspace.getConfiguration('cis');
            if (config.get<boolean>('showNotifications')) {
                vscode.window.showInformationMessage(
                    `DAG "${dagId}" 已开始运行`,
                    '查看任务'
                ).then(selection => {
                    if (selection === '查看任务') {
                        vscode.commands.executeCommand('cisTasks.focus');
                    }
                });
            }
        }
    });

    const runDagWithArgsCommand = vscode.commands.registerCommand('cis.runDagWithArgs', async (item: DagTreeItem | { dag: { id: string } }) => {
        const dagId = item instanceof DagTreeItem ? item.dag.id : item.dag.id;
        
        // 请求用户输入参数
        const argsInput = await vscode.window.showInputBox({
            prompt: `输入 DAG "${dagId}" 的参数（JSON 格式）`,
            placeHolder: '{"key": "value"}'
        });
        
        if (argsInput === undefined) {
            return; // 用户取消
        }
        
        let args: Record<string, unknown> = {};
        if (argsInput.trim()) {
            try {
                args = JSON.parse(argsInput);
            } catch (e) {
                vscode.window.showErrorMessage('参数格式错误，请使用有效的 JSON');
                return;
            }
        }
        
        const taskId = await cisApi.runDag(dagId, args);
        if (taskId) {
            providers.taskProvider.refresh();
        }
    });

    const showDagStatusCommand = vscode.commands.registerCommand('cis.showDagStatus', async (item: DagTreeItem) => {
        const status = await cisApi.getDagStatus(item.dag.id);
        if (status) {
            const panel = vscode.window.createWebviewPanel(
                'cisDagStatus',
                `DAG: ${item.dag.name}`,
                vscode.ViewColumn.One,
                {}
            );
            
            panel.webview.html = `
                <!DOCTYPE html>
                <html>
                <head>
                    <style>
                        body { font-family: var(--vscode-font-family); padding: 20px; }
                        .status-badge { 
                            display: inline-block; 
                            padding: 4px 8px; 
                            border-radius: 4px; 
                            font-size: 12px;
                            text-transform: uppercase;
                        }
                        .status-idle { background: var(--vscode-badge-background); }
                        .status-running { background: var(--vscode-debugIcon-startForeground); }
                        .status-success { background: var(--vscode-testing-iconPassed); }
                        .status-error { background: var(--vscode-testing-iconFailed); }
                        .info-row { margin: 10px 0; }
                        .label { font-weight: bold; color: var(--vscode-descriptionForeground); }
                    </style>
                </head>
                <body>
                    <h1>${item.dag.name}</h1>
                    <span class="status-badge status-${status.status}">${status.status}</span>
                    <div class="info-row"><span class="label">ID:</span> ${item.dag.id}</div>
                    <div class="info-row"><span class="label">描述:</span> ${item.dag.description || '无'}</div>
                    ${status.lastRun ? `<div class="info-row"><span class="label">上次运行:</span> ${status.lastRun}</div>` : ''}
                </body>
                </html>
            `;
        }
    });

    // 任务相关命令
    const showTaskLogsCommand = vscode.commands.registerCommand('cis.showTaskLogs', async (item: TaskTreeItem) => {
        const logs = await cisApi.getTaskLogs(item.task.id);
        
        // 创建输出通道显示日志
        const logChannel = vscode.window.createOutputChannel(`CIS Task: ${item.task.name}`);
        logChannel.clear();
        logChannel.appendLine(`任务: ${item.task.name}`);
        logChannel.appendLine(`ID: ${item.task.id}`);
        logChannel.appendLine(`状态: ${item.task.status}`);
        logChannel.appendLine('---');
        logChannel.appendLine(logs || '暂无日志');
        logChannel.show();
    });

    // 记忆搜索命令
    const searchMemoryCommand = vscode.commands.registerCommand('cis.searchMemory', async () => {
        const query = await vscode.window.showInputBox({
            prompt: '搜索记忆',
            placeHolder: '输入搜索关键词...'
        });
        
        if (!query) {
            return;
        }
        
        const results = await cisApi.searchMemory(query);
        
        if (results.length === 0) {
            vscode.window.showInformationMessage('未找到相关记忆');
            return;
        }
        
        // 显示搜索结果
        const items = results.map((r: any) => ({
            label: r.title || r.id,
            description: r.content?.substring(0, 100) || '',
            detail: `相关度: ${r.score || 'N/A'}`
        }));
        
        const selected = await vscode.window.showQuickPick(items, {
            placeHolder: `找到 ${results.length} 条相关记忆`
        });
        
        if (selected) {
            // 可以在这里添加查看详细内容的逻辑
            vscode.window.showInformationMessage(`选中: ${selected.label}`);
        }
    });

    // 初始化项目命令
    const initProjectCommand = vscode.commands.registerCommand('cis.initProject', async () => {
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (!workspaceFolders) {
            vscode.window.showErrorMessage('请先打开一个工作区');
            return;
        }
        
        const rootPath = workspaceFolders[0].uri.fsPath;
        const cisDir = vscode.Uri.file(`${rootPath}/.cis`);
        
        try {
            // 创建 .cis 目录
            await vscode.workspace.fs.createDirectory(cisDir);
            
            // 创建默认配置文件
            const configContent = Buffer.from(`[node]
name = "${vscode.workspace.name || 'my-project'}"
role = "worker"

[ai]
provider = "kimi"
# api_key = "your-api-key"

[memory]
enabled = true
`, 'utf8');
            
            await vscode.workspace.fs.writeFile(
                vscode.Uri.file(`${cisDir.fsPath}/config.toml`),
                configContent
            );
            
            // 创建 dags 目录
            await vscode.workspace.fs.createDirectory(vscode.Uri.file(`${rootPath}/dags`));
            
            // 创建示例 DAG
            const exampleDag = Buffer.from(`# 示例 DAG
[dag]
name = "hello-world"
description = "Hello World 示例"

[step.greet]
command = "echo 'Hello from CIS!'"
`, 'utf8');
            
            await vscode.workspace.fs.writeFile(
                vscode.Uri.file(`${rootPath}/dags/hello-world.dag.toml`),
                exampleDag
            );
            
            vscode.window.showInformationMessage('CIS 项目初始化完成！');
            
            // 打开配置文件
            const doc = await vscode.workspace.openTextDocument(`${cisDir.fsPath}/config.toml`);
            await vscode.window.showTextDocument(doc);
            
        } catch (error) {
            vscode.window.showErrorMessage(`初始化失败: ${error}`);
        }
    });

    // 打开设置命令
    const openSettingsCommand = vscode.commands.registerCommand('cis.openSettings', () => {
        vscode.commands.executeCommand('workbench.action.openSettings', 'cis');
    });

    // 注册所有命令
    context.subscriptions.push(
        connectCommand,
        disconnectCommand,
        refreshCommand,
        runDagCommand,
        runDagWithArgsCommand,
        showDagStatusCommand,
        showTaskLogsCommand,
        searchMemoryCommand,
        initProjectCommand,
        openSettingsCommand
    );
}
