// Package panelcore 是 panelcore.aar 的依赖锚点模块。
//
// 本 module 不含自定义代码，panelcore.aar 即官方 sing-box libbox 的
// gomobile 绑定（构建脚本：apps/android/scripts/build-panel-core.sh）。
// 保留独立 module 的原因：gomobile bind 的 gobind 用 packages.Load 解析目标包，
// 要求目标包及其绑定生成依赖出现在当前 module 的 require 列表中；
// 此处的 blank import 用于把它们钉进 go.mod。
package panelcore

import (
	_ "github.com/sagernet/gomobile"
	_ "github.com/sagernet/sing-box/experimental/libbox"
)
