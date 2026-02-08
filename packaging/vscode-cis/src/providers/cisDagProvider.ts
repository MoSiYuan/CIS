import * as vscode from 'vscode';
import { CisApiClient, DagInfo } from '../utils/cisApi';

export class CisDagProvider implements vscode.TreeDataProvider<DagTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<DagTreeItem | undefined | null | void> = new vscode.EventEmitter<DagTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<DagTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private cisApi: CisApiClient;

    constructor(cisApi: CisApiClient) {
        this.cisApi = cisApi;
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: DagTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: DagTreeItem): Promise<DagTreeItem[]> {
        if (!this.cisApi.isConnected()) {
            return [];
        }

        if (element) {
            return [];
        }

        const dags = await this.cisApi.getDags();
        return dags.map(dag => new DagTreeItem(dag));
    }

    getDagById(dagId: string): DagTreeItem | undefined {
        // 这个方法可以通过缓存实现，简化起见先返回 undefined
        return undefined;
    }
}

export class DagTreeItem extends vscode.TreeItem {
    constructor(public readonly dag: DagInfo) {
        super(dag.name || dag.id, vscode.TreeItemCollapsibleState.None);

        this.contextValue = 'dag';
        this.description = dag.description || dag.status;
        this.tooltip = `${dag.name}\nID: ${dag.id}\n状态: ${dag.status}\n${dag.lastRun ? `上次运行: ${dag.lastRun}` : ''}`;

        // 根据状态设置图标
        switch (dag.status) {
            case 'running':
                this.iconPath = new vscode.ThemeIcon('play-circle', new vscode.ThemeColor('debugIcon.startForeground'));
                break;
            case 'success':
                this.iconPath = new vscode.ThemeIcon('check', new vscode.ThemeColor('testing.iconPassed'));
                break;
            case 'error':
                this.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                break;
            default:
                this.iconPath = new vscode.ThemeIcon('debug-pause');
        }

        // 添加运行按钮
        this.command = {
            command: 'cis.showDagStatus',
            title: '查看状态',
            arguments: [this]
        };
    }
}
