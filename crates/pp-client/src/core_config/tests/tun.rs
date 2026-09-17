//! TUN 入站构建测试（`build_singbox_tun_inbound`）。

use super::features::singbox_features;
use super::*;

/// `tun_stack = "go"` 时序列化**不含** `stack` 字段（sing-box 迁移指南：移除该字段即
/// 启用 sing-tun 自研栈；序列化 "go" 会在 1.15+ 被废弃警告）；gVisor 等旧栈照常写。
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
    let tun = build_singbox_tun_inbound(&gvisor, true);
    assert_eq!(tun["stack"], "gvisor");
}
