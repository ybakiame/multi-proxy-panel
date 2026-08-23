# ProxyPanel 文档中心

欢迎查阅 ProxyPanel 官方文档。本文档涵盖架构设计、开发指南、API 参考、部署说明和贡献规范。

---

## 快速导航

| 文档 | 面向读者 | 内容概述 |
|------|----------|----------|
| [架构文档](architecture.md) | 架构师、技术负责人 | 系统架构、组件交互、数据流、数据库设计、扩展点 |
| [开发指南](development.md) | 开发者 | 环境搭建、日常开发流、数据库开发、前端开发、测试、调试 |
| [API 参考](api_reference.md) | 前端开发者、集成者 | 完整的 REST API 端点说明、请求/响应示例、错误码 |
| [部署指南](deployment.md) | 运维工程师 | Docker/手动部署、高可用、安全配置、监控、备份恢复、升级 |
| [贡献指南](contributing.md) | 贡献者 | 提交规范、PR 流程、代码审查、发布流程 |

---

## 项目概览

ProxyPanel 是一个现代化的代理节点集中管理面板，采用 **Rust** 全栈构建，基于 **Hub-Agent** 架构：

- **Hub** (`pp-hub`) — 中央管理面板，提供 REST API + gRPC 双向流服务
- **Agent** (`pp-agent`) — 节点代理，管理 sing-box/mihomo 核心进程
- **Web** (`pp-web`) — React 前端管理界面
- **CLI** (`pp-cli`) — 管理命令行工具

### 核心能力

- 多节点统一管理（心跳、状态监控）
- 多协议支持（VLESS/VMess/Trojan/SS2022/Hysteria2/TUIC）
- 双核心兼容（sing-box + mihomo）
- 自动订阅生成（Base64/JSON/Clash/SingBox/V2RayNG）
- 实时流量统计与主机性能监控
- 配置热重载与远程核心控制

---

## 快速开始

```bash
# 1. 克隆项目
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd proxy-panel

# 2. 启动数据库
docker compose up -d postgres

# 3. 初始化数据库
cargo run --bin proxy-panel -- init-db \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"

# 4. 启动 Hub
cargo run --release --bin proxy-panel-hub

# 5. 构建前端
cd crates/pp-web && bun install && bun run build
```

更多详情请参阅 [开发指南](development.md)。

---

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端框架 | Axum (HTTP) + Tonic (gRPC) |
| 异步运行时 | Tokio |
| 数据库 | Sea-ORM (PostgreSQL / SQLite) |
| 前端框架 | React 18 + TypeScript + Vite 6 + HeroUI + Tailwind CSS v4 |
| 序列化 | Serde + Protobuf |
| 观测 | Tracing + Metrics |

---

## 获取帮助

- 遇到问题？先查阅 [故障排查](deployment.md#故障排查)
- 发现 Bug？提交 [GitHub Issue](https://github.com/ybakiame/multi-proxy-panel/issues)
- 想参与开发？阅读 [贡献指南](contributing.md)
- 需要讨论？使用 [GitHub Discussions](https://github.com/ybakiame/multi-proxy-panel/discussions)

---

<p align="center">Made with ❤️ by ProxyPanel Contributors</p>
