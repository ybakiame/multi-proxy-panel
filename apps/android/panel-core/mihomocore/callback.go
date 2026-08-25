package mihomocore

// Callback 由 Kotlin 实现（gomobile 生成 Java interface）。
//
// gomobile bind 类型约束：接口方法参数/返回值只能是基本类型、string、[]byte、error 或接口。
type Callback interface {
	// Protect 回调 VpnService.protect，使出站连接绕过 VPN。
	Protect(fd int) bool
	// WriteLog 输出核心日志（level: 0=debug 1=info 2=warn 3=error）。
	WriteLog(level int, message string)
}
