# ADR-0017: 管理面板采用 shadcn/ui 与 Base UI

- **Status:** Accepted
- **Date:** 2026-10-25
- **Deciders:** 维护者确认完整迁移以提升长期定制能力
- **Scope:** apps/panel、前端依赖与主题、相关开发文档

## Context

管理面板需要持续调整复杂表格、筛选表单、弹窗和移动导航。HeroUI 3 已提供可访问组件，
但组件行为与样式主要由外部包决定，深度调整需要覆盖库实现。完整替换组件库会影响全部页面，
因此不能以局部视觉偏好为由长期混用两套体系。

## Decision

管理面板完整迁移到 shadcn/ui 的 Base UI 实现。基础组件源码通过官方 CLI 生成并纳入
`apps/panel/src/components/ui/`，由 `components.json` 管理；Tailwind CSS v4 继续提供样式与语义主题。
所有页面、共享表单、表格、弹窗和导航移除 HeroUI 依赖。迁移保留既有 API、路由、国际化和业务状态，
同时让移动导航使用具备焦点管理的 Sheet，并补齐分页国际化与可访问名称。

## Consequences

项目可以直接修改组件结构、行为和样式，长期定制不再受外部组件 API 限制；相应组件维护与升级
审查责任也由项目承担。新增组件使用 shadcn CLI，修改生成组件时保持 Base UI 的键盘和焦点语义。
不保留 HeroUI 运行时依赖或双组件体系。前端仍以构建、lint、格式检查和真实浏览器交互作为验证边界。

## Alternatives considered

保留 HeroUI 仅做易用性修复成本更低，但不满足维护者要求的长期源码级定制目标。渐进共存可降低单次
改动量，却会扩大主题、依赖和行为一致性的维护成本，因此选择一次完整迁移。
