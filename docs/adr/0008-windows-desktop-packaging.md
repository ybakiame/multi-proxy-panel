# ADR-0008: Windows 桌面端打包与分发方案

- **Status:** Accepted
- **Date:** 2026-10-05
- **Deciders:** ProxyPanel Contributors
- **Scope:** `apps/client/src-tauri/tauri.conf.json`、`.github/workflows/release.yml`、`crates/pp-client`（cores 种子释放）、客户端安装包内容
- **Related:** ADR-0007（单壳双目标）、ADR-0005（配置切片，含规则集市场 opt-in 的收敛决策）

---

## 1. Context

`apps/client` 的 Windows 桌面端在 ADR-0007 合并后已有一条最低可用的打包链路
（`bundle.targets = ["nsis"]` + release.yml `desktop-windows` job 经 tauri-action 产出
NSIS 安装包），但若干关键分发决策从未被正式记录，且首启体验存在已知短板：

1. **首启强依赖 GitHub 可达性**：sing-box 核心（`data_dir/cores/sing-box/<version>/`）与
   `wintun.dll` 均运行时下载，核心下载链路只有「GitHub Releases + 用户配置的
   `github_proxy_prefix`」两条路（`crates/pp-client/src/cores/mod.rs`）。首次启动、尚未
   配置代理前缀的用户在 GitHub 直连不可达的网络下无法完成核心安装，应用不可用。
2. **无自动更新**：未接入 `tauri-plugin-updater`，每次发版用户须手动下载安装包。
3. **配置项未显式声明**：WebView2 引导模式、NSIS 安装模式（per-user / per-machine）、
   目标架构矩阵（仅 x86_64）均为 Tauri 默认值或 CI 现状，未做决策记录。
4. **无代码签名**：未签名安装包触发 SmartScreen 警告（此前在 `docs/development.md`
   标注为预期行为，但未上升至 ADR 级别决策）。

候选打包形态对比：MSI（WiX）面向企业 GPO/MDM 批量部署，强制管理员权限、构建慢、
体积大，本项目个人用户场景无收益；便携版并非真绿色（WebView2 依赖仍在）。NSIS
体积小、per-user 安装免 UAC、为同类代理客户端（Clash Verge / v2rayN 等）主流选择，
与现有「核心运行时下载、TUN 运行时单独 UAC 提权」架构完全匹配，**无换方案理由**。

## 2. Decision

**维持 Tauri NSIS 打包方案**，补齐以下决策：

### D1 WebView2 引导：embedBootstrapper

`bundle.windows.webviewInstallMode` 显式设为 `embedBootstrapper`（安装包内嵌 WebView2
引导程序，离线可装）。目标用户网络环境下优于 `downloadBootstrapper`（安装时联网下载）。

### D2 自动更新：接入 tauri-plugin-updater

- 壳引入 `tauri-plugin-updater` + 前端 `@tauri-apps/plugin-updater`；
- release.yml 的 `desktop-windows` job 生成 updater 签名清单（`latest.json`），随
  GitHub Release 发布；
- updater 使用 ed25519 密钥对签名校验（私钥存 CI secrets），**与代码签名证书无关**；
- 更新检查经 GitHub Releases 端点，遵循客户端既有的 `github_proxy_prefix` 代理前缀
  语义（更新下载是用户态行为，可以走代理路径，与核心启动前置条件不同）。

### D3 代码签名：维持不签名（接受 SmartScreen）

不购买证书、不接入 Azure Trusted Signing，安装包不签名；SmartScreen 警告为预期行为，
在下载页与文档中向用户说明。理由：个人/开源分发场景，签名成本（金钱 + 身份审核 +
CI  secrets 管理）大于收益；updater 的 ed25519 签名校验已覆盖更新链路的完整性。

### D4 架构矩阵：增加 Windows ARM64

`desktop-windows` job 增加 `aarch64-pc-windows-msvc` target（matrix 化），NSIS 双架构
产物。D5 的种子核心按架构分别内置。

### D5 安装包内置种子核心（seed 语义）

安装包内置 **sing-box 核心 + `wintun.dll`**，解决首启强依赖 GitHub 可达性（§1-1）：

- **打包形态**：`bundle.resources` 打进安装目录，**不使用 Tauri sidecar**——sidecar
  语义是"随主进程 spawn 的伴随子进程"，与 `cores/sing-box/<version>/` 的版本化目录
  管理模型不匹配；
- **seed 语义**：仅当本地 cores 目录无任何已装核心时，首启释放内置核心到
  `data_dir/cores/sing-box/<version>/` 并登记版本；后续升级仍走现有运行时下载通道
  （`crates/pp-client/src/cores`），内置与下载两套机制不互相覆盖；
- **版本锁定与校验**：内置核心版本在仓库中锁定（构建脚本/清单记录版本号），CI 构建
  时下载并校验 SHA256 后打入安装包；
- **wintun.dll 一并内置**（仅 Windows 包）：否则 TUN 首启仍需联网拉取 wintun.net，
  种子不完整；wintun 许可证允许再分发；
- **许可证合规**：sing-box 为 GPL-3.0，随二进制分发须在安装包/关于页附带 GPL-3.0
  许可证文本与上游源码指引（项目本身 AGPL 开源，源码可得性已满足）。

### D6 NSIS 安装模式：仅 per-user

仅提供 per-user 安装（免 UAC），不提供 per-machine 选项。TUN 的管理员需求由运行时
「以管理员重启」机制覆盖（`docs/development.md`），安装侧不引入 UAC。

### D7 国家分流：维持规则集市场 opt-in，不回摆

明确**不**随安装包内置国家规则集种子（geoip-cn / geosite-cn 等 .srs），也**不**恢复
内置 CN 分流规则。维持 ADR-0005 时期（2026-09）的收敛决策：内置规则仅「私有地址
直连」一条（`geosite-private` / `geoip-private`），国家分流由用户从规则集市场
（`metaCubeCatalog`）自选添加，下载走 `ruleset_manager` 既有的
jsDelivr → GitHub raw → 用户代理前缀镜像回退与降级启动链路。

## 3. Consequences

**正面**：

- 首启离线可用（D5），消除 GitHub 可达性这一最大首启阻塞点；
- 发版后用户可自动更新（D2），分发闭环；
- ARM64 Windows 设备得到一等支持（D4）；
- 打包决策全部显式化（D1/D3/D6），后续变更可追溯。

**代价与风险**：

- 安装包体积增加约 10MB+（压缩态 sing-box 核心 + wintun.dll）；
- 内置核心版本随时间陈旧——seed 语义下仅影响全新安装的首启版本，运行时下载通道
  承担升级，可接受；需在发版流程中定期提升锁定版本；
- GPL-3.0 再分发合规义务（许可证文本 + 源码指引）成为发布物的一部分，遗漏即违规；
- 不签名（D3）意味着 SmartScreen 警告长期存在，用户沟通成本转移到文档与下载页；
- D2 引入 ed25519 私钥管理职责（CI secrets 泄露 = 可推送恶意更新），需按密钥管理
  规范保管。

**文档同步**：落地时更新 `docs/development.md`（Windows 构建与首启行为）、
`docs/deployment.md` / `README.md`（下载与安装说明，含 SmartScreen 说明）、
`CHANGELOG.md`。

## 4. Alternatives considered

- **MSI（WiX）**：企业部署友好，但强制管理员权限、构建慢、体积大，个人用户场景无
  收益，拒绝。
- **便携版（zip / 单 exe）**：WebView2 依赖仍在，并非真绿色；可作未来补充产物，本轮
  不做。
- **sidecar 形态内置核心**：与 cores 版本化目录管理冲突（sidecar 无版本语义、固定
  随主进程），拒绝，选 `bundle.resources` + seed。
- **内置国家规则集种子 / 恢复内置 CN 分流**：见 D7——与 ADR-0005 的收敛决策冲突，
  维持 opt-in；若未来首启分流体验仍被诟病，以独立 ADR 回摆。
- **Azure Trusted Signing / OV 代码签名**：可消除 SmartScreen，但引入费用与身份审核，
  本轮明确不做（D3），未来预算允许时可单独评估。
