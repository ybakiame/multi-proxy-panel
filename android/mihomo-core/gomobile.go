package mihomocore

// 与 sing-box AndroidLib 构建（cmd/internal/build_libbox）一致：
// gobind 生成绑定代码时用 packages.Load 解析 "github.com/sagernet/gomobile/bind"，
// 要求该 module 出现在本 module 的 require 列表中。根包是零开销 stub，
// 仅用于把依赖钉进 go.mod（gobind 独立进程解析用）。
import _ "github.com/sagernet/gomobile"
