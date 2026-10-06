/// PanelError 体积回归守护：clippy::result_large_err 阈值为 128 字节，
/// PanelError 必须保持在该阈值之下（tonic::Status 等大 variant 一律装箱）。
#[test]
fn panel_error_stays_under_clippy_threshold() {
    assert!(
        std::mem::size_of::<pp_common::PanelError>() <= 128,
        "PanelError 为 {} 字节，超过 clippy::result_large_err 阈值 128：请把大 variant 装箱",
        std::mem::size_of::<pp_common::PanelError>()
    );
}
