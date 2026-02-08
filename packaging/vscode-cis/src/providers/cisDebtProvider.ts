import * as vscode from 'vscode';
import { CisApiClient, DebtInfo } from '../utils/cisApi';

export class CisDebtProvider implements vscode.TreeDataProvider<DebtTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<DebtTreeItem | undefined | null | void> = new vscode.EventEmitter<DebtTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<DebtTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private cisApi: CisApiClient;

    constructor(cisApi: CisApiClient) {
        this.cisApi = cisApi;
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: DebtTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: DebtTreeItem): Promise<DebtTreeItem[]> {
        if (!this.cisApi.isConnected()) {
            return [];
        }

        if (element) {
            return [];
        }

        const debts = await this.cisApi.getDebts();
        return debts.map(debt => new DebtTreeItem(debt));
    }
}

export class DebtTreeItem extends vscode.TreeItem {
    constructor(public readonly debt: DebtInfo) {
        super(debt.description.substring(0, 50), vscode.TreeItemCollapsibleState.None);

        this.contextValue = 'debt';
        this.description = `${debt.type} - ${debt.severity}`;
        this.tooltip = `类型: ${debt.type}\n严重程度: ${debt.severity}\n描述: ${debt.description}\n创建时间: ${debt.createdAt}`;

        // 根据严重程度设置图标
        switch (debt.severity) {
            case 'critical':
                this.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('errorForeground'));
                break;
            case 'high':
                this.iconPath = new vscode.ThemeIcon('warning', new vscode.ThemeColor('editorWarning.foreground'));
                break;
            case 'medium':
                this.iconPath = new vscode.ThemeIcon('info', new vscode.ThemeColor('editorInfo.foreground'));
                break;
            default:
                this.iconPath = new vscode.ThemeIcon('circle-outline');
        }
    }
}
