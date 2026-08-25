# ProxyPanel 部署文件

本目录提供 systemd 服务单元文件，用于在 Linux 服务器上长期运行 ProxyPanel Hub 与 Agent。

## 快速部署（推荐）

### 部署 Hub

使用 bootstrap 脚本一键安装：

```bash
curl -fsSL https://raw.githubusercontent.com/ybakiame/multi-proxy-panel/main/scripts/install-hub.sh | bash -s --
```

脚本会自动完成：架构检测、CLI 下载与 SHA256 校验、委托 `proxy-panel install hub` 完成安装。安装后请按提示编辑 `/etc/proxy-panel/hub.toml` 配置数据库，然后执行 `proxy-panel init-db` 与 `systemctl enable --now proxy-panel-hub`。

### 部署 Agent

在 Web 面板「节点管理」中点击「安装指令」按钮，复制命令到节点服务器执行：

```bash
curl -fsSL https://<your-hub-domain>/install.sh | bash -s -- \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token" \
  --name "node-01"
```

安装脚本会自动完成：架构检测、CLI 下载与 SHA256 校验、委托 `proxy-panel install agent` 完成安装并启动服务。支持 `--uninstall` / `--purge` 卸载与 `--version` 指定版本。

## 常用 CLI 运维命令

```bash
# 查看状态
sudo proxy-panel status

# 升级
sudo proxy-panel upgrade hub
sudo proxy-panel upgrade agent
sudo proxy-panel upgrade cli

# 回滚
sudo proxy-panel rollback hub
sudo proxy-panel rollback agent

# 日志
sudo proxy-panel logs hub --lines 100 --follow
sudo proxy-panel logs agent --lines 50

# 重启
sudo proxy-panel restart hub
sudo proxy-panel restart agent

# 卸载
sudo proxy-panel uninstall agent --purge
sudo proxy-panel uninstall hub --purge
```

详见 [docs/deployment.md](../docs/deployment.md)。

---

## 手动部署（备用）

若因环境限制无法使用一键安装，可按以下步骤手动部署。

### 文件说明

| 文件 | 说明 |
|------|------|
| `clash-full-template.json` | Clash Meta 完整配置订阅模板（占位符方式） |
| `proxy-panel-hub.service` | Hub 服务单元 |
| `proxy-panel-agent.service` | Agent 服务单元 |

### 前置准备

1. 创建运行用户：

   ```bash
   sudo useradd -r -s /bin/false proxypanel
   ```

2. 准备目录：

   ```bash
   sudo mkdir -p /opt/proxy-panel/{bin,web/dist,backup}
   sudo mkdir -p /var/lib/proxy-panel/agent
   sudo mkdir -p /etc/proxy-panel
   sudo chown -R proxypanel:proxypanel /opt/proxy-panel /var/lib/proxy-panel
   ```

3. 复制二进制文件（通过 `cargo build --release --workspace` 构建或从 Release 下载）：

   ```bash
   sudo cp target/release/proxy-panel-hub /usr/local/bin/
   sudo cp target/release/proxy-panel-agent /usr/local/bin/
   sudo cp target/release/proxy-panel /usr/local/bin/
   ```

4. 复制前端产物：

   ```bash
   cd crates/pp-web && bun install && bun run build
   sudo cp -r crates/pp-web/dist/* /opt/proxy-panel/web/dist/
   ```

5. 准备配置文件：

   ```bash
   sudo cp config/hub.example.toml /etc/proxy-panel/hub.toml
   # 编辑 /etc/proxy-panel/hub.toml，填入数据库、JWT secret 等
   ```

## 安装服务

```bash
sudo cp deploy/proxy-panel-hub.service /etc/systemd/system/
sudo cp deploy/proxy-panel-agent.service /etc/systemd/system/
sudo systemctl daemon-reload
```

## 启动 Hub

```bash
# 初始化数据库（首次部署）
sudo proxy-panel init-db --database-url "postgres://..."

# 创建首个管理员
sudo proxy-panel create-user \
  --database-url "postgres://..." \
  --username admin \
  --password "STRONG_PASSWORD"

# 启动并启用 Hub
sudo systemctl enable --now proxy-panel-hub
sudo proxy-panel logs hub --follow
```

## 启动 Agent

在节点服务器上创建 `/etc/proxy-panel/agent.env`：

```bash
PROXYPANEL_HUB_URL=https://grpc.example.com:50052
PROXYPANEL_AGENT_TOKEN=your-agent-token
```

然后启动：

```bash
sudo systemctl enable --now proxy-panel-agent
sudo proxy-panel logs agent --follow
```

## 状态查看

```bash
sudo proxy-panel status
sudo systemctl status proxy-panel-hub
sudo systemctl status proxy-panel-agent
sudo proxy-panel logs hub --lines 100
```

## 自动化部署

推荐使用 Release 产物 + `proxy-panel` CLI 完成自动化部署（无需本地构建）：

### 部署 Hub

```bash
curl -fsSL https://github.com/ybakiame/multi-proxy-panel/releases/latest/download/install-hub.sh | sudo bash -s -- --version latest
```

或先安装 CLI 再执行：

```bash
sudo proxy-panel install hub --version latest
# 按提示编辑 /etc/proxy-panel/hub.toml 填入 database_url
sudo proxy-panel init-db --database-url "postgres://..."
sudo systemctl enable --now proxy-panel-hub
```

### 部署 Agent

在面板「节点管理 → 添加节点」生成一键接入命令，或手动执行：

```bash
curl -fsSL https://<your-hub-domain>/install.sh | sudo bash -s -- \
  --hub-url "https://<your-hub-domain>:443" \
  --token "<节点 token>" \
  --name "<节点名称>"
```

日常生命周期管理（升级 / 回滚 / 卸载 / 状态 / 日志）统一使用：

```bash
sudo proxy-panel upgrade hub --version vX.Y.Z
sudo proxy-panel rollback hub
sudo proxy-panel status
```

## 订阅模板

### Clash 完整配置模板

`deploy/clash-full-template.json` 是一个 Clash Meta / Mihomo 完整配置模板，使用占位符机制：

- `"\u003cPROXY_REPLACE\u003e"` — 注入节点 proxy 列表
- `"\u003cNODE_REPLACE\u003e"` — 注入节点名称列表

通过 API 创建到 Hub：

```bash
export PANEL_API_KEY=your_api_key
export PANEL_HOST=https://test3-panel.ybakiame.net
python3 scripts/create-clash-template.py
```

创建后，在 Web 界面新建/编辑订阅，选择 `clash-full` 模板即可生成完整客户端配置。

## 完整部署指南

详见 [docs/deployment.md](../docs/deployment.md)。
