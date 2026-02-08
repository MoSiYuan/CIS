import * as vscode from 'vscode';
import { CisNodeProvider } from './providers/cisNodeProvider';
import { CisDagProvider } from './providers/cisDagProvider';
import { CisTaskProvider } from './providers/cisTaskProvider';
import { CisDebtProvider } from './providers/cisDebtProvider';
import { CisCodeLensProvider } from './providers/cisCodeLensProvider';
import { CisApiClient } from './utils/cisApi';
import { registerCommands } from './commands';

let cisApi: CisApiClient | undefined;
let outputChannel: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext) {
    outputChannel = vscode.window.createOutputChannel('CIS');
    outputChannel.appendLine('CIS 扩展已激活');

    // 读取配置
    const config = vscode.workspace.getConfiguration('cis');
    const nodeAddress = config.get<string>('nodeAddress') || 'http://localhost:7676';
    const autoConnect = config.get<boolean>('autoConnect') ?? true;

    // 创建 API 客户端
    cisApi = new CisApiClient(nodeAddress, outputChannel);

    // 注册视图提供器
    const nodeProvider = new CisNodeProvider(cisApi);
    const dagProvider = new CisDagProvider(cisApi);
    const taskProvider = new CisTaskProvider(cisApi);
    const debtProvider = new CisDebtProvider(cisApi);

    vscode.window.registerTreeDataProvider('cisNodes', nodeProvider);
    vscode.window.registerTreeDataProvider('cisDags', dagProvider);
    vscode.window.registerTreeDataProvider('cisTasks', taskProvider);
    vscode.window.registerTreeDataProvider('cisDebts', debtProvider);

    // 注册 CodeLens 提供器
    const dagDocumentSelector: vscode.DocumentSelector = [
        { pattern: '**/*.dag.toml' },
        { pattern: '**/dags/**/*.toml' }
    ];
    
    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider(
            dagDocumentSelector,
            new CisCodeLensProvider()
        )
    );

    // 注册命令
    registerCommands(context, cisApi, {
        nodeProvider,
        dagProvider,
        taskProvider,
        debtProvider
    }, outputChannel);

    // 自动连接
    if (autoConnect) {
        vscode.commands.executeCommand('cis.connect');
    }

    // 设置定时刷新
    const refreshInterval = config.get<number>('refreshInterval') || 5000;
    const interval = setInterval(() => {
        if (cisApi?.isConnected()) {
            nodeProvider.refresh();
            dagProvider.refresh();
            taskProvider.refresh();
        }
    }, refreshInterval);

    context.subscriptions.push({
        dispose: () => clearInterval(interval)
    });

    outputChannel.appendLine('CIS 扩展初始化完成');
}

export function deactivate() {
    outputChannel?.appendLine('CIS 扩展已停用');
    cisApi?.disconnect();
}

export { cisApi, outputChannel };
