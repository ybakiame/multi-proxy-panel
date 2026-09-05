# 包管理器操作规则（Package Manager Operations）

> 解决问题：直接文本编辑依赖清单（package.json / Cargo.toml / go.mod …）导致锁文件失步、版本解析不一致、workspace 依赖关系被破坏。

## 核心规则

**依赖的添加 / 删除 / 更新一律调用对应包管理器的命令，禁止用文本编辑方式手改清单文件来增删依赖。**

| 生态 | 清单 / 锁文件 | 添加 | 删除 | 更新 |
|------|--------------|------|------|------|
| JavaScript / Node | `package.json` + `bun.lock` 等 | `bun add <pkg>`（dev 依赖 `bun add -d`） | `bun remove <pkg>` | `bun update <pkg>` |
| Rust | `Cargo.toml` + `Cargo.lock` | `cargo add <crate>`（`-p` 指定 workspace 成员） | `cargo remove <crate>` | `cargo update -p <crate>` |
| Python | `pyproject.toml` + `uv.lock` | `uv add <pkg>` | `uv remove <pkg>` | `uv lock --upgrade-package <pkg>` |
| Go | `go.mod` + `go.sum` | `go get <module>@<ver>` | `go mod tidy`（删 import 后清理） | `go get -u <module>` |
| Java / Kotlin | `pom.xml` / `build.gradle(.kts)` | 见下方例外说明 | 同左 | `./mvnw versions:use-latest-releases` / `./gradlew` 系命令 |

- 项目同时存在多个包管理器时，**以该子项目 lockfile 对应的那个为准**（本仓库前端是 Bun workspaces：一律用 `bun`，不要用 npm/pnpm/yarn 混用，否则产生多份锁文件）。
- pip 场景（无 uv 的老项目）也走命令：`pip install <pkg>` 后按需 `pip freeze` 类命令回写约束文件，而不是先改文件再装。

## 细则

1. **锁文件禁止手改**：`bun.lock`、`package-lock.json`、`Cargo.lock`、`uv.lock`、`go.sum`、`gradle.lockfile` 等一律由命令生成；合并冲突时用包管理器命令重新生成，不手工拼接。
2. **手改清单后必须补跑解析命令**：Gradle/Maven 等没有统一 add 命令、确实需要编辑 `build.gradle.kts`/`pom.xml` 的场景，改完必须立即运行对应构建命令（如 `./gradlew dependencies` / `./mvnw dependency:resolve`）验证解析通过并刷新锁文件。
3. **workspace 项目在根部操作**：Bun workspaces 在仓库根部 `bun add --filter <pkg-name> <dep>`（或在对应 app 目录执行），保证单一 `bun.lock`；Cargo workspace 用 `cargo add -p <crate> <dep>`，不要直接编辑成员的 `Cargo.toml`。
4. **版本约束交给命令写**：需要特定版本时用命令参数表达（`bun add pkg@^1.2.0`、`cargo add crate@1.2`），让工具按自身格式写入约束，不手写版本字符串猜格式。
5. **删依赖要删干净**：`remove` 之后确认 import/use 已无残留引用；Go 用 `go mod tidy` 收尾。
6. **装完即验证**：依赖变更后跑一次该子项目的构建/检查命令（如 `bun run --filter pp-web verify`、`cargo check -p <crate>`），确认解析与编译通过再提交；锁文件变更必须随清单变更加入同一个提交。

## 规则联动

- 提交粒度见根目录 `AGENTS.md` 第 5 节（原子化提交：依赖变更与被依赖的代码改动同属一个逻辑单元时一起提交）
- 前端依赖验证命令见 `AGENTS.md` 第 2.3 节
