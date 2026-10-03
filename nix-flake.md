# ProxyPanel 开发环境（Nix flake）

本仓库通过 `flake.nix` 声明式管理全部开发依赖：Rust 生态工具、Android SDK/NDK、Go（panelcore）、Bun 前端、桌面 GUI 系统库。任何一台新机器 `nix develop` 即可获得与 CI/真机验证一致的环境。

---

## 1. 前置：安装 Nix 并启用 Flakes

```sh
sh <(curl -L https://nixos.org/nix/install) --daemon   # 官方脚本（daemon 模式）
mkdir -p ~/.config/nix
echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
source /etc/profile && nix --version                    # 需 2.11+
```

WSL2 用户：flakes 需要 systemd 或 daemon 模式均可，`/etc/profile` 需含 nix profile 路径。

## 2. 使用

```bash
nix develop                 # 进入开发 shell（首次会构建/下载声明式 Android SDK，约 1-2GB）
nix develop -c bash -c 'adb devices; go version; bun --version'   # 冒烟
exit                        # 退出
```

建议配合 direnv（`.envrc` 中写 `use flake`）实现进入目录自动激活。

## 3. 覆盖的依赖清单

| 类别 | 内容 | 来源 |
|---|---|---|
| Android | **SDK 35+36 / build-tools 35.0.0+36.0.0 / platform-tools / NDK 28.0.13004108**（`cmdline-tools` 齐备；许可证已预接受。Android 工程 compileSdk=36 需要 platform/build-tools 36） | `tadfisher/android-nixpkgs`（stable 频道） |
| Android 工具链 | JDK 17（JAVA_HOME 统一指向）、gradle、cargo-ndk | nixpkgs |
| Rust | rustup + `rust-toolchain.toml` 锁定（stable + rustfmt/clippy + aarch64-linux-android target） | rustup（见下「Rust 例外」） |
| Go | go 1.25.x（panelcore / gomobile 构建） | nixpkgs |
| 前端 | Bun、Node.js（Bun workspaces 管理） | nixpkgs |
| 桌面 GUI 系统库 | webkit2gtk-4.1、gtk3、glib-networking、openssl、alsa-lib、xorg 等（Tauri 客户端与 `pp-client-tauri` 的 host 链接与运行） | nixpkgs |
| 构建基础 | pkg-config、cmake、ninja、perl（openssl vendored）、protobuf、gnumake | nixpkgs |

环境变量由 shell 自动导出：`ANDROID_HOME` / `ANDROID_SDK_ROOT` / `ANDROID_NDK_ROOT` / `NDK_HOME` / `ANDROID_NDK_HOME`（均指向 nix store 中的 NDK 28.0.13004108）/ `ANDROID_JAR`（platforms-android-35）/ `JAVA_HOME` / `PKG_CONFIG_PATH`（flake 系统库优先）。另按 aarch64 单 ABI（Android 目标仅发布 arm64）导出 `CC_*` / `AR_*` / `CARGO_TARGET_*_LINKER` / `BINDGEN_EXTRA_CLANG_ARGS_*`，使裸 `cargo check/build --target aarch64-linux-android` 也走 nix NDK；非 nix 环境的同一组变量由 `apps/client/scripts/android-ndk-env.sh` 注入（被 `scripts/check-rust-gates.sh` 引用）。仓库内不再有任何写死的 NDK 绝对路径。

## 4. 例外与注意事项

1. **Rust 工具链本体走 rustup**（不进 nix）：`rust-toolchain.toml` 是版本锁定的事实来源，rustup 按其自动安装 stable + components + `aarch64-linux-android` target。进入 shell 后 `cargo`/`rustc` 由 rustup 代理。
2. **首次 `nix develop`** 会从 dl.google.com 下载 Android SDK/NDK 组合（1-2GB，一次性），之后全部走本地 store 缓存。
3. **adb 设备**：宿主机已运行的 adb server 与 shell 内 nix 版 adb 版本一致即可混用；遇 `no devices` 先 `adb kill-server; adb devices`。
4. **WSL2/WSLg**：GUI（Tauri 客户端窗口）依赖 WSLg 提供的 Wayland/X11；GPU 加速走宿主驱动。
5. `direnv` 可选：`.envrc` 写 `use flake` 后进入仓库目录自动激活。
6. **用户 shell rc 覆盖陷阱**：`nix develop` 的交互 bash 会 source `~/.bashrc`，若其中无条件 `export ANDROID_HOME/NDK_HOME` 指向主机 SDK，会覆盖 flake 的 nix 路径（tauri 探测 NDK 优先读 `NDK_HOME`），表现为「进了 nix 却仍用主机 NDK」。请在 rc 中用 `[ -z "$IN_NIX_SHELL" ]` 守卫这些导出（本机 `~/.bashrc` 已加）。

## 5. 更新依赖

```bash
nix flake update            # 更新 nixpkgs / android-nixpkgs 锁定
nix develop                 # 重新构建并验证
```

Android SDK 组件的新版本（如 NDK r29）由 android-nixpkgs stable 频道每日跟随 Google 仓库更新，改 `flake.nix` 中 `ndk-28-0-13004108` 为目标版本后 `nix flake update` 即可。

---

## 历史说明

本文件最初为 KMP 迁移期的 Nix 入门笔记（2026-09-19），现升级为项目正式开发依赖管理文档。文中历史方案（KMP + Compose Multiplatform → Dioxus 全栈 Rust）最终由 Tauri 2 客户端方案取代（见 ADR-0007 与 `docs/adoption/audit-pass1.md` 的记录）；原 `docs/adr/0006-mobile-client-dioxus.md` 已不存在，现行 `docs/adr/0006-client-traffic-stats.md` 复用了该编号记录流量统计决策。
