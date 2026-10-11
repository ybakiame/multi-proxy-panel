# MITM 证书查询响应性修复

日期：2026-10-11。

根因：同步 Tauri 命令在窗口线程执行证书 IO、certutil 等系统信任查询。
CA 读取、安装和信任查询改为 async 命令，通过 spawn_blocking 执行；前端信任
状态缓存五分钟、提供刷新与查询提示，安装前取消旧查询以避免旧状态覆盖结果。

后台并发首次读取 CA 时，FileCaStore 使用进程内锁保护证书和私钥加载/生成，
避免查询与核心启动生成不同的 CA 材料。

Nix 环境验证：cargo build --workspace、bun run verify:rust（fmt、workspace
clippy/test、壳 host clippy、Android target check）通过。新增并发回归测试
concurrent_stores_share_one_complete_ca_pair 通过：八个独立 store 同时首次读取，
返回材料均与最终磁盘证书/私钥一致。pp-client-app verify 通过。

浏览器 mock 的延迟信任查询显示查询态，其他页面可导航；该检查只能证明前端
交互不等待查询，真实 Windows certutil 的窗口响应性仍需测试构建真机验收。
