# CIS VS Code 插件

[CIS (Cluster of Independent Systems)](https://github.com/MoSiYuan/CIS) 的 Visual Studio Code 扩展，提供 DAG 管理、任务监控、记忆搜索等功能。

## 功能特性

### 🌲 侧边栏视图

- **节点视图**: 显示本地节点状态和对等节点列表
- **DAG 视图**: 浏览和管理所有 DAG，一键运行
- **任务视图**: 实时监控任务执行状态和进度
- **债务视图**: 查看技术债务和优化建议

### 🚀 快捷操作

- 在 DAG 文件中显示 **运行按钮** (CodeLens)
- 快捷键运行当前 DAG (`Ctrl+Shift+R` / `Cmd+Shift+R`)
- 快速搜索记忆 (`Ctrl+Shift+M` / `Cmd+Shift+M`)

### 🔔 实时通知

- DAG 运行完成通知
- 任务状态变化提醒
- 错误告警

## 安装

### 从 VS Code 市场安装（推荐）

1. 打开 VS Code
2. 进入扩展视图 (`Ctrl+Shift+X`)
3. 搜索 "CIS"
4. 点击安装

### 从 VSIX 文件安装

```bash
# 1. 下载 VSIX 文件
wget https://github.com/MoSiYuan/CIS/releases/download/v1.1.0/vscode-cis-0.1.0.vsix

# 2. 在 VS Code 中安装
code --install-extension vscode-cis-0.1.0.vsix
```

### 从源码构建

```bash
cd packaging/vscode-cis
npm install
npm run compile
npm run package
```

## 配置

打开 VS Code 设置 (`Ctrl+,`)，搜索 "cis"：

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `cis.nodeAddress` | `http://localhost:7676` | CIS 节点 HTTP API 地址 |
| `cis.wsAddress` | `ws://localhost:6767` | CIS WebSocket 地址 |
| `cis.apiKey` | `` | API 密钥（如需要） |
| `cis.autoConnect` | `true` | 启动时自动连接 |
| `cis.showNotifications` | `true` | 显示 DAG 完成通知 |
| `cis.refreshInterval` | `5000` | 刷新间隔（毫秒） |

## 使用方法

### 1. 连接 CIS 节点

- 点击侧边栏的 CIS 图标
- 点击 "连接节点" 按钮
- 或运行命令 `CIS: 连接节点`

### 2. 运行 DAG

**方式一**: 在 DAG 视图中点击 DAG 旁边的播放按钮

**方式二**: 打开 DAG 文件，点击 CodeLens 中的 "▶ 运行 DAG"

**方式三**: 使用快捷键 `Ctrl+Shift+R`

### 3. 监控任务

- 在任务视图中查看所有任务
- 点击任务查看详细日志
- 实时刷新显示进度

### 4. 搜索记忆

- 使用快捷键 `Ctrl+Shift+M`
- 或运行命令 `CIS: 搜索记忆`
- 输入关键词进行语义搜索

### 5. 初始化项目

- 运行命令 `CIS: 初始化 CIS 项目`
- 自动生成 `.cis/` 目录和示例 DAG

## DAG 文件支持

插件自动识别以下文件作为 DAG 配置文件：

- `*.dag.toml`
- `dags/**/*.toml`

在这些文件中，插件会显示：

- 运行按钮
- 状态指示器
- 参数输入提示

## 快捷键

| 快捷键 | 命令 |
|--------|------|
| `Ctrl+Shift+R` | 运行当前 DAG |
| `Ctrl+Shift+M` | 搜索记忆 |
| `Ctrl+Shift+P` → 输入 "CIS" | 查看所有 CIS 命令 |

## 故障排除

### 无法连接节点

1. 检查 CIS 节点是否运行：`cis node status`
2. 确认地址配置正确
3. 检查网络连接

### DAG 不显示

1. 确认文件扩展名为 `.dag.toml`
2. 检查文件是否在 `dags/` 目录中
3. 刷新视图

### 日志不显示

1. 检查 CIS 版本是否支持日志 API
2. 查看输出面板中的 CIS 通道

## 开发

```bash
# 安装依赖
npm install

# 编译
npm run compile

# 调试
按 F5 启动调试

# 打包
npm run package

# 发布
npm run publish
```

## 贡献

欢迎提交 Issue 和 PR！

- [GitHub 仓库](https://github.com/MoSiYuan/CIS)
- [问题反馈](https://github.com/MoSiYuan/CIS/issues)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE)
