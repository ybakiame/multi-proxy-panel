// Package mihomocore provides a gomobile-friendly wrapper around the mihomo core.
//
// Fallback maintenance mode: this package is used as a silent fallback kernel when
// sing-box cannot parse a subscription. Only crash fixes are accepted; no new
// features or protocol support will be added.
package mihomocore

// Callback is implemented by Kotlin (gomobile-generated Java interface).
//
// gomobile bind type constraints: interface method parameters/return values can
// only be basic types, string, []byte, error, or interfaces.
type Callback interface {
	// Protect forwards to VpnService.protect so outbound connections bypass the VPN.
	Protect(fd int) bool
	// WriteLog outputs core logs (level: 0=debug 1=info 2=warn 3=error).
	WriteLog(level int, message string)
}
