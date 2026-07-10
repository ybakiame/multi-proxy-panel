# ProxyPanel 部署文件

本目录提供 systemd 服务单元文件，用于在 Linux 服务器上长期运行 ProxyPanel Hub 与 Agent。

## 文件说明

| 文件 | 说明 |
|------|------|
| `proxy-panel-hub.service` | Hub 服务单元 |
| `proxy-panel-agent.service` | Agent 服务单元 |

## 前置准备

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

## 完整部署指南

详见 [docs/deployment.md](../docs/deployment.md)。
