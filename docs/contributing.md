# ProxyPanel 贡献指南

感谢您对 ProxyPanel 的兴趣！本文档描述如何参与项目贡献。

---

## 目录

1. [行为准则](#行为准则)
2. [如何贡献](#如何贡献)
3. [开发环境](#开发环境)
4. [提交规范](#提交规范)
5. [Pull Request 流程](#pull-request-流程)
6. [代码审查](#代码审查)
7. [文档贡献](#文档贡献)
8. [发布流程](#发布流程)
9. [联系方式](#联系方式)

---

## 行为准则

- 尊重所有参与者，保持友善和建设性的沟通
- 接受建设性批评，专注于技术讨论
- 禁止任何形式的歧视、骚扰或攻击性言行
- 优先社区利益，而非个人利益

---

## 如何贡献

### 报告问题

在提交 Issue 前，请：

1. 搜索现有 Issue，避免重复
2. 使用最新的 `main` 分支复现问题
3. 提供最小复现步骤

**Bug 报告模板：**

```markdown
## 环境信息
- OS: Ubuntu 22.04
- Rust: 1.88.0
- ProxyPanel: v0.1.0 (commit: abc123)
- Database: PostgreSQL 16

## 复现步骤
1. 启动 Hub
2. 创建节点
3. 推送配置

## 预期行为
配置应成功推送到 Agent

## 实际行为
收到 502 Bad Gateway 错误

## 日志
```
[ERROR] failed to push config: agent not connected
```
```

### 功能建议

欢迎提出新功能建议！请：

1. 描述使用场景和痛点
2. 提出可能的解决方案
3. 讨论实现的可行性和影响范围

### 代码贡献

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交代码变更
4. 确保测试通过
5. 提交 Pull Request

---

## 开发环境

参见 [development.md](development.md) 获取详细的开发环境搭建指南。

快速开始：

```bash
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd proxy-panel
docker compose up -d postgres
cargo run --bin proxy-panel -- init-db --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"
cargo build --workspace
```

---

## 提交规范

### 原子化提交 (Atomic Commits)

**每个提交应只包含一个逻辑变更单元**，确保提交历史清晰、可回滚、易于审查和 `git bisect` 定位问题。

**原则：**

1. **单一职责**：一个提交只做一件事（如：修复一个 Bug、添加一个功能、重构一个模块）
2. **独立可编译**：每个提交后项目应能正常编译通过（`cargo build --workspace`）
3. **独立可测试**：每个提交后相关测试应能通过（`cargo test --workspace`）
4. **逻辑完整性**：不要把不相关的改动混在一起（如：修复 Bug 时不要顺手格式化无关文件）

**正确示例：**

```bash
# ✅ 独立的功能提交
git commit -m "feat(hub): 添加节点批量导入 API"

# ✅ 独立的修复提交
git commit -m "fix(agent): 修复断连后指数退避计算溢出的问题"

# ✅ 独立的重构提交
git commit -m "refactor(db): 提取节点查询为 NodeService 方法"

# ✅ 独立的文档提交
git commit -m "docs: 更新部署指南中的 TLS 配置示例"
```

**错误示例：**

```bash
# ❌ 混合多个不相关的变更
# 一个提交里同时：修复了 API 路由、格式化了代码、更新了 README
git commit -m "fix: 一些问题"

# ❌ 半成品提交
# 提交了未完成的代码，导致项目无法编译
git commit -m "wip: 工作中"
```

**提交前自检清单：**

- [ ] 这个提交是否只解决了一个明确的问题或实现了一个明确的功能？
- [ ] 如果现在要回滚这个提交，是否不会影响其他正常功能？
- [ ] `git diff --cached` 显示的变更是否都是必要的？
- [ ] 无关的改动（如 IDE 配置、临时文件）是否已排除？

**处理中途工作的策略：**

如果需要暂存未完成的工作，使用 `git stash` 或草稿分支，而不是提交到功能分支：

```bash
# 暂存当前工作
git stash push -m "WIP: 实现流量聚合逻辑"

# 切换处理紧急修复
git checkout -b hotfix/critical-bug
# ... 修复并提交 ...

# 回到原功能分支继续工作
git checkout feature/traffic-aggregation
git stash pop
```

### Conventional Commits 格式

使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

### 类型 (Type)

| 类型 | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 文档变更 |
| `style` | 代码格式（不影响功能） |
| `refactor` | 重构 |
| `perf` | 性能优化 |
| `test` | 测试相关 |
| `chore` | 构建/工具变更 |
| `ci` | CI/CD 配置 |
| `security` | 安全修复 |

### 范围 (Scope)

可选，指明变更的模块：

- `hub` — Hub 服务
- `agent` — Agent 服务
- `web` — Web 前端
- `cli` — CLI 工具
- `db` — 数据库层
- `config` — 配置构建器
- `core` — 核心管理
- `sub` — 订阅生成器
- `proto` — gRPC 协议
- `common` — 共享模块
- `docs` — 文档

### 示例

```
feat(hub): 添加节点批量导入 API

支持通过 CSV/JSON 批量导入节点，减少管理员操作时间。

Closes #123
```

```
fix(agent): 修复断连后无法自动重连的问题

当 Hub 重启时，Agent 的重连逻辑存在竞态条件，
导致无限等待。添加超时和指数退避重试。

Fixes #456
```

```
docs: 更新部署指南中的 Nginx 配置示例

添加 gRPC 代理配置和 TLS 设置说明。
```

---

## Pull Request 流程

### 创建 PR

1. 确保分支基于最新的 `main`
2. 运行完整测试套件：`cargo test --workspace`
3. 运行代码检查：`cargo clippy --workspace --all-targets -- -D warnings`
4. 格式化代码：`cargo fmt --all`
5. 填写 PR 模板：

```markdown
## 变更说明
描述本次 PR 的目的和变更内容。

## 类型
- [ ] Bug 修复
- [ ] 新功能
- [ ] 破坏性变更
- [ ] 文档更新
- [ ] 性能优化
- [ ] 代码重构

## 检查清单
- [ ] 代码遵循项目规范
- [ ] 测试已添加或更新
- [ ] 文档已更新
- [ ] CHANGELOG 已更新（如适用）
- [ ] 手动测试通过

## 相关 Issue
Fixes #123
Closes #456
```

### PR 审查标准

审查者将关注：

- **正确性**: 代码逻辑是否正确
- **测试**: 是否有足够的测试覆盖
- **文档**: 公开 API 是否有文档注释
- **性能**: 是否有明显的性能问题
- **安全**: 是否有安全隐患
- **兼容性**: 是否破坏向后兼容

### 合并策略

- 使用 **Squash and Merge** 保持历史清晰
- PR 标题应符合提交规范
- 需要至少 1 个审查者批准
- CI 检查必须通过

---

## 代码审查

### 审查者指南

作为审查者，请：

1. **及时响应**: 尽量在 48 小时内响应审查请求
2. **具体明确**: 指出具体行号和问题，提供改进建议
3. **区分优先级**: 使用 `nit:`（小建议）、`suggestion:`（建议）、`blocking:`（必须修复）标记
4. **保持尊重**: 对代码不对人，解释 "为什么" 而非仅指出问题

### 作者指南

作为 PR 作者，请：

1. **保持耐心**: 审查是协作过程，不是个人批评
2. **解释意图**: 对复杂逻辑添加注释说明设计决策
3. **响应及时**: 尽快回复审查意见，必要时讨论而非直接接受
4. **小步快跑**: 优先提交小而专注的 PR，而非大而全的变更

---

## 文档贡献

文档与代码同等重要！

### 文档位置

| 文档 | 路径 | 说明 |
|------|------|------|
| README | `README.md` | 项目概览和快速开始 |
| AGENTS | `AGENTS.md` | 开发代理指南 |
| 架构文档 | `docs/architecture.md` | 系统架构说明 |
| 开发指南 | `docs/development.md` | 开发环境和工作流 |
| API 参考 | `docs/api_reference.md` | REST API 文档 |
| 部署指南 | `docs/deployment.md` | 生产部署说明 |
| 贡献指南 | `docs/contributing.md` | 本文档 |

### 文档规范

- 使用 Markdown 格式
- 标题使用 ATX 风格 (`#`)
- 代码块标注语言
- 保持中英文术语一致性
- 更新目录时同步更新文内 TOC

---

## 发布流程

### 版本号规范

使用 [SemVer](https://semver.org/lang/zh-CN/)：`MAJOR.MINOR.PATCH`

- `MAJOR`: 不兼容的 API 变更
- `MINOR`: 向下兼容的功能新增
- `PATCH`: 向下兼容的问题修复

### 发布步骤

1. 更新 `Cargo.toml` 中的版本号
2. 更新 `CHANGELOG.md`
3. 创建 Git Tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
4. 推送 Tag: `git push origin v0.1.0`
5. 在 GitHub 创建 Release，填写变更说明
6. CI 自动构建并发布二进制文件

### CHANGELOG 格式

```markdown
## [0.2.0] - 2024-02-15

### Added
- 新增 Hysteria2 协议支持
- 添加节点批量导入功能
- 支持自定义订阅模板

### Changed
- 优化配置推送性能，减少 50% 延迟
- 升级依赖: tokio 1.45, axum 0.8

### Fixed
- 修复 Agent 断连后偶发崩溃问题 (#789)
- 修复订阅链接凭证注入错误 (#790)

### Security
- 更新 rustls 到 0.23.5，修复潜在漏洞
```

---

## 联系方式

- **Issue 追踪**: [GitHub Issues](https://github.com/ybakiame/multi-proxy-panel/issues)
- **讨论区**: [GitHub Discussions](https://github.com/ybakiame/multi-proxy-panel/discussions)
- **安全报告**: 请发送邮件至 security@example.com（不要公开披露安全漏洞）

---

## 致谢

感谢所有为 ProxyPanel 做出贡献的开发者！

<a href="https://github.com/ybakiame/multi-proxy-panel/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=ybakiame/multi-proxy-panel" />
</a>
