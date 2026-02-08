import * as vscode from 'vscode';

export interface NodeInfo {
    id: string;
    name: string;
    status: 'online' | 'offline' | 'error';
    peers: number;
    version: string;
}

export interface DagInfo {
    id: string;
    name: string;
    description?: string;
    status: 'idle' | 'running' | 'error' | 'success';
    lastRun?: string;
}

export interface TaskInfo {
    id: string;
    name: string;
    status: 'pending' | 'running' | 'completed' | 'failed';
    progress: number;
    dagId?: string;
}

export interface DebtInfo {
    id: string;
    type: string;
    description: string;
    severity: 'low' | 'medium' | 'high' | 'critical';
    createdAt: string;
}

export class CisApiClient {
    private baseUrl: string;
    private outputChannel: vscode.OutputChannel;
    private connected: boolean = false;

    constructor(baseUrl: string, outputChannel: vscode.OutputChannel) {
        this.baseUrl = baseUrl.replace(/\/$/, '');
        this.outputChannel = outputChannel;
    }

    isConnected(): boolean {
        return this.connected;
    }

    async connect(): Promise<boolean> {
        try {
            const response = await fetch(`${this.baseUrl}/health`);
            this.connected = response.ok;
            
            if (this.connected) {
                this.outputChannel.appendLine(`已连接到 CIS 节点: ${this.baseUrl}`);
                vscode.commands.executeCommand('setContext', 'cis.connected', true);
            }
            
            return this.connected;
        } catch (error) {
            this.connected = false;
            this.outputChannel.appendLine(`连接失败: ${error}`);
            vscode.commands.executeCommand('setContext', 'cis.connected', false);
            return false;
        }
    }

    disconnect(): void {
        this.connected = false;
        vscode.commands.executeCommand('setContext', 'cis.connected', false);
        this.outputChannel.appendLine('已断开连接');
    }

    async getNodeInfo(): Promise<NodeInfo | undefined> {
        if (!this.connected) return undefined;
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/node/info`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`获取节点信息失败: ${error}`);
            return undefined;
        }
    }

    async getPeers(): Promise<NodeInfo[]> {
        if (!this.connected) return [];
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/network/peers`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`获取对等节点失败: ${error}`);
            return [];
        }
    }

    async getDags(): Promise<DagInfo[]> {
        if (!this.connected) return [];
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/dags`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`获取 DAG 列表失败: ${error}`);
            return [];
        }
    }

    async runDag(dagId: string, args?: Record<string, unknown>): Promise<string | undefined> {
        if (!this.connected) {
            vscode.window.showErrorMessage('未连接到 CIS 节点');
            return undefined;
        }
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/dags/${dagId}/run`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(args || {})
            });
            
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            const result = await response.json();
            
            vscode.window.showInformationMessage(`DAG "${dagId}" 已开始运行`);
            return result.taskId;
        } catch (error) {
            vscode.window.showErrorMessage(`运行 DAG 失败: ${error}`);
            return undefined;
        }
    }

    async getDagStatus(dagId: string): Promise<DagInfo | undefined> {
        if (!this.connected) return undefined;
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/dags/${dagId}/status`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`获取 DAG 状态失败: ${error}`);
            return undefined;
        }
    }

    async getTasks(): Promise<TaskInfo[]> {
        if (!this.connected) return [];
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/tasks`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`获取任务列表失败: ${error}`);
            return [];
        }
    }

    async getTaskLogs(taskId: string): Promise<string> {
        if (!this.connected) return '';
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/tasks/${taskId}/logs`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            const data = await response.json();
            return data.logs || '';
        } catch (error) {
            this.outputChannel.appendLine(`获取任务日志失败: ${error}`);
            return '';
        }
    }

    async getDebts(): Promise<DebtInfo[]> {
        if (!this.connected) return [];
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/debts`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`获取债务列表失败: ${error}`);
            return [];
        }
    }

    async searchMemory(query: string): Promise<unknown[]> {
        if (!this.connected) return [];
        
        try {
            const response = await fetch(`${this.baseUrl}/api/v1/memory/search?q=${encodeURIComponent(query)}`);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            return await response.json();
        } catch (error) {
            this.outputChannel.appendLine(`搜索记忆失败: ${error}`);
            return [];
        }
    }
}
