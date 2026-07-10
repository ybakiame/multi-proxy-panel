# ProxyPanel Hub

中央管理面板，提供 HTTP REST API、gRPC 双向流服务，并托管 Web 前端静态文件。

## 构建

```bash
# Debug
cargo build --bin proxy-panel-hub

# Release
cargo build --release --bin proxy-panel-hub
```

产物：`target/release/proxy-panel-hub`

## 运行

```bash
# 使用默认配置 config/hub.toml
./target/release/proxy-panel-hub

# 指定配置与静态文件目录
./target/release/proxy-panel-hub \
  --config /etc/proxy-panel/hub.toml \
  --static-dir /opt/proxy-panel/web/dist
```

## 环境变量

所有配置项均可通过 `PROXYPANEL_` 前缀的环境变量覆盖，例如：

```bash
PROXYPANEL_DATABASE_URL="postgres://..."
PROXYPANEL_JWT_SECRET="..."
PROXYPANEL_HUB_LISTEN="0.0.0.0:8081"
PROXYPANEL_GRPC_LISTEN="0.0.0.0:50052"
```

## 启动流程

1. 准备 PostgreSQL 数据库
2. 复制并编辑 `config/hub.example.toml` → `config/hub.toml`
3. 运行 `proxy-panel init-db` 应用迁移
4. 运行 `proxy-panel create-user` 创建管理员
5. 启动 Hub
6. 构建并放置前端产物到 `static_dir`

## 接口

| 端点 | 说明 |
|------|------|
| `http://localhost:8081` | HTTP API + Web 面板 |
| `http://localhost:50052` | gRPC Agent 服务 |
| `/health` | 健康检查 |
| `/metrics` | Prometheus 指标 |

## 测试

```bash
cargo test -p pp-hub
```

## 部署

生产部署请使用 systemd 服务文件，参见 [deploy/README.md](../deploy/README.md) 和 [docs/deployment.md](../docs/deployment.md)。
