# ADR-0018: Hub 受控自更新与部署授权模型

- **Status:** Accepted
- **Date:** 2026-10-25
- **Deciders:** 维护者
- **Scope:** `pp-cli`、`pp-hub`、`apps/panel`、`deploy/`、`docs/`
- **Related:** ADR-0016（日志持久化）、`docs/deployment.md`（CLI 生命周期管理）

## Context

ProxyPanel 的组件升级此前完全依赖管理员登录 Hub 服务器手动执行
`sudo proxy-panel upgrade hub`。CLI（见 `b8aa459`）已具备稳定的发布检测
（`check-update`）、事务性升级（下载 → 校验 → 备份 → 替换 → 重启 → 失败尝试回滚）、
`rollback` 命令、`maintenance.lock` 非阻塞锁与 `--status-file` JSON 状态输出；
Web 面板（见 `5983459`）需要一个入口把这套能力暴露给管理员，而不把 Hub 进程
变成高权限进程。

关键约束：

- Hub 服务以非特权 `proxypanel` 用户运行，且 systemd unit 带
  `NoNewPrivileges=true` 等加固；升级二进制、重启服务需要 root，跨越真实
  root/systemd 边界。
- 面板 API Key 体系已按 scope 细分，新增 `system:read` / `system:write` scope。
- 不能因新增面板入口而降低既有部署默认安全姿态：授权必须显式、可选、可审计。
- 升级对象是 Hub 自身：更新进程若在 Hub unit 内运行，Hub 停止/重启会连带杀掉
  未完成的更新。

## Decision

1. **CLI 是唯一的升级执行者**：版本检测、下载校验、备份替换、失败时的恢复尝试、
   回滚全部收敛在 `proxy-panel` CLI（`upgrade` / `rollback` / `check-update`）。
   CLI 以 root 运行，install / upgrade / rollback 共用 `/opt/proxy-panel/maintenance.lock`
   非阻塞锁防止并发维护；`upgrade --status-file` 原子写入
   `running` / `succeeded` / `failed` / `rolled_back` JSON 状态。
2. **Hub 面板只调度、不执行**：新增 `GET/POST /api/v1/system/update` 与
   `GET /api/v1/system/update/status`，scope 分别为 `system:read` / `system:write`。
   POST 通过受限 sudo 规则调用固定形状的命令：
   `sudo /usr/bin/systemd-run --unit=proxy-panel-hub-update --collect
   /usr/local/bin/proxy-panel upgrade hub --version <tag> --repo <repo>
   --status-file /opt/proxy-panel/data/hub-update.json`。
   transient unit 使更新进程脱离 `proxy-panel-hub.service` 的 cgroup，Hub 停止或
   被重启不会杀死进行中的更新；`--collect` 保证 unit 退出后回收。
3. **默认关闭**：`maintenance_enabled` 默认为 `false`。未启用时 POST 返回 403，
   GET 仍执行检测并返回 `manual_command`，面板 `/system` 页照常显示检测结果与
   可复制的手动命令。
4. **授权是可选的管理员动作**：仓库只提供样例文件
   `deploy/proxy-panel-maintenance.sudoers`（仅允许 proxypanel 以 root 无密码执行
   上述固定形状命令）与 `deploy/proxy-panel-hub-maintenance.conf` drop-in
   （仅 `[Service] NoNewPrivileges=false`）。安装脚本**不**自动安装它们；
   基础 service 的加固不做任何修改。NoNewPrivileges 放宽使 `sudo` 提权可行，
   风险由 sudoers 条目的固定参数形状约束。
5. **状态文件恢复**：更新状态持久化于 `maintenance_status_file`
   （默认 `data/hub-update.json`），Hub 重启后从文件恢复；`running` 状态超过
   30 分钟标记为 `stale`，要求人工核验后再操作。内存中的
   `MaintenanceState`（AtomicBool）防止单实例内并发触发，文件状态防止跨重启
   重复触发。
6. **本次不做 Agent 远程升级**：面板只支持 Hub 自更新。Agent 组件的升级仍需
   在各节点本地以 root 执行 CLI；跨区域远程 root 执行（经 gRPC 下发 shell）
   超出当前信任边界，后续若需要应单独评估（如带签名的更新指令 + Agent 侧
   白名单校验）。

## Consequences

- 面板更新中心在默认部署中是"检测 + 手动命令提示"模式，授权安装后升级为
  "一键触发"模式；两种模式共享同一 API。
- 备份仅保留最新一份（见 v0.4.5 变更）：升级失败的恢复能力是"尽力恢复"，
  回滚与失败自动回滚都不能保证必然成功，文档与面板提示必须如实说明，管理员
  应在升级前自行备份数据库与配置。
- `NoNewPrivileges=false` drop-in 放宽了 Hub 进程的提权防护面；缓解措施是
  sudoers 条目把可执行命令限制为固定参数形状，且 unit 名称、状态文件路径均
  不可由 API 请求改变（路径来自 Hub 配置而非请求体）。
- 状态文件路径必须与 sudoers 中 `--status-file` 参数一致，否则 sudo 拒绝执行；
  文档在部署指南中给出对照说明。
- Docker / 容器化部署不适用该授权模型（容器内通常无 systemd/sudo），应沿用
  镜像滚动升级。

## Alternatives considered

- **Hub 进程以 root 运行或新增 setuid 辅助程序**：放弃。直接扩大常驻进程权限
  违背了最小权限原则，出错/被利用的爆炸半径远大于受限 sudo 条目。
- **Agent 代为升级 Hub**：放弃。Agent 与 Hub 可能不同机；让分布式节点持有 Hub
  所在机的 root 执行通道会引入更大的横向风险。
- **轮询 GitHub 缓存最新版本**：放弃写库，保留每次 GET 实时查询。发布检测
  频率低（管理员手动查看），实时查询更简单且避免缓存失效语义；代价是
  GitHub 不可达时 GET 返回 502，面板提示手动命令不受影响。
