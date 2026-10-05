fn main() {
    // 显式静态链接 VC++ 运行时（等价于已废弃的 STATIC_VCRUNTIME 环境变量；
    // 当前 CLI 版本的 tauri.conf.json schema 尚不接受 build.windows.staticVCRuntime，
    // 故走 build.rs 属性通道）。默认值本就为 true，此处显式固定以防上游默认值漂移。
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new().static_vc_runtime(true)),
    )
    .expect("error while running tauri build script");
}
