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
