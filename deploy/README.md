# ProxyPanel 部署文件

本目录提供 systemd 服务单元文件，用于在 Linux 服务器上长期运行 ProxyPanel Hub 与 Agent。

## 快速部署 Agent（推荐）

Hub 提供一键安装脚本，无需手动准备 systemd 文件：

```bash
# 在 Web 面板「节点管理」中点击「安装指令」按钮，复制命令到节点服务器执行
curl -fsSL https://<your-hub-domain>/install.sh | bash -s -- \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token" \
  --name "node-01"
```

安装脚本会自动完成：架构检测、二进制下载与 SHA256 校验、systemd 服务创建与启动。支持 `--uninstall` / `--purge` 卸载与 `--version` 指定版本。详见 [docs/deployment.md](../docs/deployment.md#节点一键安装推荐)。

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
   sudo mkdir -p /opt/proxy-panel/{bin,config,web,data}
   sudo mkdir -p /var/lib/proxy-panel/agent
   sudo chown -R proxypanel:proxypanel /opt/proxy-panel /var/lib/proxy-panel
   ```

3. 复制二进制文件（通过 `cargo build --release --workspace` 构建）：

   ```bash
   sudo cp target/release/proxy-panel-hub /usr/local/bin/
   sudo cp target/release/proxy-panel-agent /usr/local/bin/
   sudo cp target/release/proxy-panel /usr/local/bin/
   ```

4. 复制前端产物：

   ```bash
   cd crates/pp-web && npm install && npm run build
   sudo cp -r crates/pp-web/dist/* /opt/proxy-panel/web/dist/
   ```

5. 准备配置文件：

   ```bash
   sudo cp config/hub.example.toml /etc/proxy-panel/hub.toml
   # 编辑 /etc/proxy-panel/hub.toml，填入数据库、JWT  secret 等
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
sudo /usr/local/bin/proxy-panel init-db --database-url "postgres://..."

# 创建首个管理员
sudo /usr/local/bin/proxy-panel create-user \
  --database-url "postgres://..." \
  --username admin \
  --password "STRONG_PASSWORD"

# 启动并启用 Hub
sudo systemctl enable --now proxy-panel-hub
sudo journalctl -u proxy-panel-hub -f
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
sudo journalctl -u proxy-panel-agent -f
```

## 状态查看

```bash
sudo systemctl status proxy-panel-hub
sudo systemctl status proxy-panel-agent
sudo journalctl -u proxy-panel-hub -n 100 --no-pager
```

## 自动化部署脚本

项目根目录 `scripts/deploy-server.py` 支持一键部署 Hub 或 Agent（基于 SSH + paramiko，自动打包、上传、安装 systemd 服务）。

### 前置要求

- 本地已完成 Release 构建：`cargo build --release --workspace`
- 前端已构建：`cd crates/pp-web && npm run build`
- Python 3 + paramiko：`pip install paramiko`

### 部署 Hub（含本地 Agent + Caddy）

```bash
python3 scripts/deploy-server.py \
  --mode hub \
  --host 192.3.150.233 \
  --password '***REMOVED***' \
  --domain test2-panel.ybakiame.net
```

### 部署 Agent（仅 Agent，连接远端 Hub）

```bash
python3 scripts/deploy-server.py \
  --mode agent \
  --host 64.188.27.110 \
  --password '***REMOVED***' \
  --hub-host test2-panel.ybakiame.net:50052 \
  --agent-token your-agent-token
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
