//! TUN 入站构建测试（`build_singbox_tun_inbound`）。

use super::features::singbox_features;
use super::*;

/// `tun_stack = "go"` 时序列化**不含** `stack` 字段（sing-box 迁移指南 Migrate TUN
/// Stack：`stack` 字段整体废弃、移除即启用 sing-tun 自研栈）；mixed/system 及
/// desktop 遗留 gvisor 照常写。
#[test]
fn build_singbox_tun_inbound_omits_stack_field_for_go() {
    let features = PanelFeatures {
        tun_stack: "go".to_string(),
        ..singbox_features()
    };
    let tun = build_singbox_tun_inbound(&features, true);
    assert!(
        tun.get("stack").is_none(),
        "go stack must omit the stack field: {tun}"
    );

    let gvisor = PanelFeatures {
        tun_stack: "gvisor".to_string(),
        ..singbox_features()
    };
    // gVisor 选项已从 mobile UI 移除（存量迁移为 go），仅 desktop 仍提供。
    let tun = build_singbox_tun_inbound(&gvisor, false);
    assert_eq!(tun["stack"], "gvisor");
}
