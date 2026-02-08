import * as vscode from 'vscode';
import { CisApiClient, NodeInfo } from '../utils/cisApi';

export class CisNodeProvider implements vscode.TreeDataProvider<NodeTreeItem> {
    private _onDidChangeTreeData: vscode.EventEmitter<NodeTreeItem | undefined | null | void> = new vscode.EventEmitter<NodeTreeItem | undefined | null | void>();
    readonly onDidChangeTreeData: vscode.Event<NodeTreeItem | undefined | null | void> = this._onDidChangeTreeData.event;

    private cisApi: CisApiClient;
    private nodes: NodeInfo[] = [];

    constructor(cisApi: CisApiClient) {
        this.cisApi = cisApi;
    }

    refresh(): void {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(element: NodeTreeItem): vscode.TreeItem {
        return element;
    }

    async getChildren(element?: NodeTreeItem): Promise<NodeTreeItem[]> {
        if (!this.cisApi.isConnected()) {
            return [new NodeTreeItem('点击连接按钮连接 CIS 节点', 'info')];
        }

        if (!element) {
            // 根节点：本地节点和对等节点
            const items: NodeTreeItem[] = [];
            
            const nodeInfo = await this.cisApi.getNodeInfo();
            if (nodeInfo) {
                items.push(new NodeTreeItem(
                    nodeInfo.name || '本地节点',
                    'local-node',
                    nodeInfo,
                    vscode.TreeItemCollapsibleState.Collapsed
                ));
            }

            const peers = await this.cisApi.getPeers();
            if (peers.length > 0) {
                items.push(new NodeTreeItem(
                    `对等节点 (${peers.length})`,
                    'peers-group',
                    undefined,
                    vscode.TreeItemCollapsibleState.Collapsed
                ));
            }

            return items;
        }

        if (element.contextValue === 'local-node' && element.nodeInfo) {
            // 本地节点的子项
            return [
                new NodeTreeItem(`ID: ${element.nodeInfo.id}`, 'info'),
                new NodeTreeItem(`版本: ${element.nodeInfo.version}`, 'info'),
                new NodeTreeItem(`状态: ${element.nodeInfo.status}`, 'info'),
                new NodeTreeItem(`对等节点: ${element.nodeInfo.peers}`, 'info')
            ];
        }

        if (element.contextValue === 'peers-group') {
            // 对等节点列表
            const peers = await this.cisApi.getPeers();
            return peers.map(peer => new NodeTreeItem(
                peer.name || peer.id,
                'peer',
                peer,
                vscode.TreeItemCollapsibleState.Collapsed
            ));
        }

        if (element.contextValue === 'peer' && element.nodeInfo) {
            // 对等节点的子项
            return [
                new NodeTreeItem(`ID: ${element.nodeInfo.id}`, 'info'),
                new NodeTreeItem(`版本: ${element.nodeInfo.version}`, 'info'),
                new NodeTreeItem(`状态: ${element.nodeInfo.status}`, 'info')
            ];
        }

        return [];
    }
}

class NodeTreeItem extends vscode.TreeItem {
    constructor(
        public readonly label: string,
        public readonly contextValue: string,
        public readonly nodeInfo?: NodeInfo,
        public readonly collapsibleState: vscode.TreeItemCollapsibleState = vscode.TreeItemCollapsibleState.None
    ) {
        super(label, collapsibleState);

        // 设置图标
        switch (contextValue) {
            case 'local-node':
                this.iconPath = new vscode.ThemeIcon('server-environment');
                this.description = '本地';
                break;
            case 'peer':
                this.iconPath = new vscode.ThemeIcon('server');
                this.description = nodeInfo?.status;
                break;
            case 'peers-group':
                this.iconPath = new vscode.ThemeIcon('group-by-ref-type');
                break;
            case 'info':
                this.iconPath = new vscode.ThemeIcon('info');
                break;
        }
    }
}
