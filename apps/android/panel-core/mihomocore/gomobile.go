package mihomocore

// Package mihomocore provides a gomobile-friendly wrapper around the mihomo core.
//
// Fallback maintenance mode: this package is used as a silent fallback kernel when
// sing-box cannot parse a subscription. Only crash fixes are accepted; no new
// features or protocol support will be added.
//
// Consistent with sing-box AndroidLib build (cmd/internal/build_libbox):
// gobind generates binding code using packages.Load to parse
// "github.com/sagernet/gomobile/bind", requiring this module to appear in the
// require list. The root package is a zero-overhead stub used only to pin the
// dependency into go.mod (for gobind's independent process resolution).
//
// libbox is similar: the second target package of gomobile bind,
// github.com/sagernet/sing-box/experimental/libbox, is not in this module,
// so it must be in the require list to be resolved by gobind's packages.Load.
import (
	_ "github.com/sagernet/gomobile"
	_ "github.com/sagernet/sing-box/experimental/libbox"
)
