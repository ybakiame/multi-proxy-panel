# ProxyPanel 配置目录

本目录存放 ProxyPanel Hub 的配置文件示例与生产配置。

## 文件说明

| 文件 | 说明 |
|------|------|
| `hub.toml` | 当前使用的 Hub 配置文件（已加入 `.gitignore`，不提交敏感信息） |
| `hub.example.toml` | 配置模板，复制为 `hub.toml` 后按需修改 |

## 快速开始

1. 复制示例配置：

   ```bash
   cp config/hub.example.toml config/hub.toml
   ```

2. 修改必填项：

   - `database_url`: PostgreSQL 连接字符串（生产必须）
   - `jwt_secret`: 至少 32 字符的随机字符串，用于签发管理员 JWT
   - `cors_origins`: Web 面板的来源域名
   - `auto_register_agents`: 生产环境必须设为 `false`

3. 使用环境变量覆盖（可选）：

   ```bash
   export PROXYPANEL_DATABASE_URL="postgres://user:pass@localhost/proxypanel"
   export PROXYPANEL_JWT_SECRET="$(openssl rand -base64 42)"
   export PROXYPANEL_CORS_ORIGINS="https://panel.example.com"
   ```

## 生产检查清单

- [ ] `jwt_secret` 已改为随机长字符串（Hub 会拒绝默认占位符）
- [ ] `database_url` 使用 PostgreSQL 而非 SQLite
- [ ] `cors_origins` 设置为精确的 Web 面板域名
- [ ] `auto_register_agents` 设为 `false`
- [ ] 配置了 `trusted_proxy_ips`（若使用反向代理）
- [ ] 通过反向代理或 `http_tls_*` 启用 HTTPS

## 更多部署信息

详见 [docs/deployment.md](../docs/deployment.md)。
