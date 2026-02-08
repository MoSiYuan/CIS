import * as vscode from 'vscode';
import { CisApiClient, TaskInfo } from '../utils/cisApi';

export class CisTaskProvider implements vscode.TreeDataProvider<TaskTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<TaskTreeItem | undefined | null | void> = new vscode.EventEmitter<TaskTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<TaskTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private cisApi: CisApiClient;

    constructor(cisApi: CisApiClient) {
        this.cisApi = cisApi;
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: TaskTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: TaskTreeItem): Promise<TaskTreeItem[]> {
        if (!this.cisApi.isConnected()) {
            return [];
        }

        if (element) {
            return [];
        }

        const tasks = await this.cisApi.getTasks();
        return tasks.map(task => new TaskTreeItem(task));
    }
}

export class TaskTreeItem extends vscode.TreeItem {
    constructor(public readonly task: TaskInfo) {
        super(task.name || task.id, vscode.TreeItemCollapsibleState.None);

        this.contextValue = 'task';
        this.description = `${task.status} (${task.progress}%)`;
        this.tooltip = `任务: ${task.name}\nID: ${task.id}\n状态: ${task.status}\n进度: ${task.progress}%`;

        // 根据状态设置图标
        switch (task.status) {
            case 'running':
                this.iconPath = new vscode.ThemeIcon('sync~spin');
                break;
            case 'completed':
                this.iconPath = new vscode.ThemeIcon('check');
                break;
            case 'failed':
                this.iconPath = new vscode.ThemeIcon('error');
                break;
            default:
                this.iconPath = new vscode.ThemeIcon('circle-outline');
        }
    }
}
