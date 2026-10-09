# 数据迁移规则（Data Migration）

> 目标：迁移逻辑可预期、可清理，不在常驻代码里堆积历史包袱。

## 1. 原则：迁移是「一次性动作」，不是「长期兼容分支」

数据形态、存储键名、schema 变更时，迁移逻辑必须**执行一次即完成**，禁止写成每次运行/每次读取都命中的兼容代码（惰性迁移、双键回退读取等）。

**反例**（本仓库真实踩坑，mobile localStorage 主题键迁移初版）：

```ts
// 每次读取都检查旧键——旧键永远活在热路径上，成为甩不掉的垃圾数据
const stored = localStorage.getItem(NEW_KEY) ?? localStorage.getItem(LEGACY_KEY);
if (stored && localStorage.getItem(NEW_KEY) === null) {
  localStorage.setItem(NEW_KEY, stored); // 惰性改写
  localStorage.removeItem(LEGACY_KEY);
}
```

**正确做法**：启动早期（模块初始化 / Provider 挂载前）执行一次性迁移函数，之后所有读写只面向新形态：

```ts
// 应用启动时执行一次：旧键存在且新键缺失 → 搬迁并删除旧键
export function migrateLegacyStorageKeys(): void {
  try {
    const legacy = localStorage.getItem(LEGACY_KEY);
    if (legacy !== null && localStorage.getItem(NEW_KEY) === null) {
      localStorage.setItem(NEW_KEY, legacy);
    }
    localStorage.removeItem(LEGACY_KEY); // 无论是否搬迁都清掉旧键
  } catch {
    /* 存储不可用：跳过，本次会话用默认值 */
  }
}
```

## 2. 分层落地

| 层 | 迁移载体 | 本项目实例 |
|----|---------|-----------|
| 服务端数据库 | **版本化升级步骤**（注册到 `crates/pp-db/src/upgrade.rs` 的 `UPGRADE_STEPS`，按 `introduced_in` 版本门控执行一次） | 清理废弃功能数据 |
| 客户端持久化（localStorage / 配置文件 / 数据库） | **启动时一次性迁移函数**（模块初始化阶段调用一次，幂等） | `pp-ui-theme` 替换 `heroui-theme` |
| 构建期/开发期数据（脚本产物、生成物） | **迁移脚本**（`scripts/` 下一次性执行，执行完即弃，可不入主干历史长期维护） | 批量改写依赖方数据 |

## 3. 细则

- **幂等**：迁移函数重复执行必须安全（已迁移则无操作）。
- **首帧前置代码例外**：`index.html` 预置脚本这类无法复用模块代码的场景，允许保留**只读回退**（读旧键兜底），但不得承担写入迁移职责；写入迁移由主代码的一次性函数完成。
- **设置删除期限**：一次性迁移代码在「旧形态用户几乎不存在」后可整体删除——在注释里写明引入版本，便于日后清理。
- **禁止**把迁移写成 UI 组件/读取函数的常驻分支（每次渲染/读取都判断旧形态）。
