# ProxyPanel CLI

管理命令行工具，用于数据库初始化、创建 API Key、创建管理员用户、生成 Token 等运维操作。

## 构建

```bash
# Debug
cargo build --bin proxy-panel

# Release
cargo build --release --bin proxy-panel
```

产物：`target/release/proxy-panel`

## 常用命令

### 初始化数据库

```bash
proxy-panel init-db --database-url "postgres://user:pass@localhost/proxypanel"
```

### 创建管理员用户

```bash
proxy-panel create-user \
  --database-url "postgres://user:pass@localhost/proxypanel" \
  --username admin \
  --password "STRONG_PASSWORD"
```

### 创建 API Key

```bash
proxy-panel create-api-key \
  --database-url "postgres://user:pass@localhost/proxypanel" \
  --name "cli-admin" \
  --scopes "*"
```

输出为原始 API Key，请妥善保存。

### 生成 Agent Token

```bash
proxy-panel agent-token --node-name "tokyo-01"
```

### 预置节点

```bash
proxy-panel provision-node \
  --database-url "postgres://user:pass@localhost/proxypanel" \
  --name "tokyo-01" \
  --hostname "tokyo-01.example.com" \
  --address "192.0.2.1"
```

输出 `node_id` 和 `token`。

### 数据库诊断

```bash
proxy-panel diagnose --database-url "postgres://user:pass@localhost/proxypanel"
```

### 检查稳定版更新

无需 root；省略组件时检查 Hub、Agent 和 CLI。CLI 未安装到 `/usr/local/bin` 时，仅在开发场景回退到当前编译版本。

```bash
proxy-panel check-update
proxy-panel check-update hub --repo ybakiame/multi-proxy-panel
proxy-panel check-update cli --json
```

输出当前版本、最新稳定版、是否有更新及 Release 页面；`--json` 输出机器可读数组。

### 升级与回滚

```bash
sudo proxy-panel upgrade hub --version latest
sudo proxy-panel upgrade agent --version 0.4.5 --repo ybakiame/multi-proxy-panel
sudo proxy-panel upgrade cli --status-file /opt/proxy-panel/upgrade-status.json
sudo proxy-panel rollback hub
```

升级会先解析稳定 Release 并完成下载、校验和解压，再备份和替换文件。相同版本会跳过，较旧目标会被拒绝（降级使用 `rollback`）。Hub 的二进制与 `web/dist` 一同备份和恢复；启动失败时会尝试从备份恢复旧版并报告结果——这是尽力恢复，不保证必然成功（备份仅保留最新一份，升级前请自行备份数据库与配置），且仅重新启动升级前处于 active 的服务。`--status-file` 原子写入 `running`、`succeeded`、`failed` 或 `rolled_back` JSON 状态。install、upgrade 和 rollback 共用 `/opt/proxy-panel/maintenance.lock` 非阻塞锁。

Hub 自更新也可经面板 `/system` 页触发：Hub 通过受限 sudo + transient systemd unit 调用本 CLI（默认关闭，授权安装见 `deploy/proxy-panel-maintenance.sudoers` 与根部署文档）。

## 环境变量

多数命令支持通过环境变量传入 `--database-url`：

```bash
export PROXYPANEL_DATABASE_URL="postgres://user:pass@localhost/proxypanel"
proxy-panel init-db
```

## 测试

```bash
cargo test -p pp-cli
```
