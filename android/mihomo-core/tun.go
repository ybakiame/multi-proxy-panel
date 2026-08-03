package mihomocore

import (
	"net"
	"net/netip"
	"strings"
	"sync"
	"syscall"

	"github.com/metacubex/mihomo/component/dialer"
	"github.com/metacubex/mihomo/constant"
	LC "github.com/metacubex/mihomo/listener/config"
	"github.com/metacubex/mihomo/listener/sing_tun"
	"github.com/metacubex/mihomo/log"
	"github.com/metacubex/mihomo/tunnel"
)

var (
	tunMux     sync.Mutex
	tunListner *sing_tun.Listener
)

// StartTun 使用 VpnService 提供的 fd 创建 sing_tun listener，并安装 Protect 钩子。
func StartTun(fd int, stack string, address string, dns string) error {
	tunMux.Lock()
	defer tunMux.Unlock()

	if tunListner != nil {
		_ = tunListner.Close()
		tunListner = nil
	}

	tunStack, ok := constant.StackTypeMapping[strings.ToLower(stack)]
	if !ok {
		tunStack = constant.TunSystem
	}

	var prefix4, prefix6 []netip.Prefix
	for _, a := range strings.Split(address, ",") {
		a = strings.TrimSpace(a)
		if a == "" {
			continue
		}
		prefix, err := netip.ParsePrefix(a)
		if err != nil {
			log.Errorln("[mihomocore] TUN: parse address %s error: %v", a, err)
			return err
		}
		if prefix.Addr().Is4() {
			prefix4 = append(prefix4, prefix)
		} else {
			prefix6 = append(prefix6, prefix)
		}
	}

	var dnsHijack []string
	for _, d := range strings.Split(dns, ",") {
		d = strings.TrimSpace(d)
		if d == "" {
			continue
		}
		dnsHijack = append(dnsHijack, net.JoinHostPort(d, "53"))
	}

	options := LC.Tun{
		Enable:              true,
		Device:              "ProxyPanel",
		Stack:               tunStack,
		DNSHijack:           dnsHijack,
		AutoRoute:           false,
		AutoDetectInterface: false,
		Inet4Address:        prefix4,
		Inet6Address:        prefix6,
		MTU:                 9000,
		FileDescriptor:      fd,
	}

	installHook()
	l, err := sing_tun.New(options, tunnel.Tunnel)
	if err != nil {
		removeHook()
		log.Errorln("[mihomocore] TUN: %v", err)
		return err
	}
	tunListner = l
	log.Infoln("[mihomocore] TUN listening: %s", l.Address())
	return nil
}

// StopTun 停止 TUN listener 并移除 Protect 钩子。
func StopTun() {
	tunMux.Lock()
	defer tunMux.Unlock()
	if tunListner != nil {
		_ = tunListner.Close()
		tunListner = nil
	}
	removeHook()
}

// installHook 注册 dialer 钩子：出站连接 fd 经 Callback.Protect 交给
// VpnService 处理，使代理流量绕过 VPN（P1 不做 per-app 进程归属）。
func installHook() {
	dialer.DefaultSocketHook = func(network, address string, conn syscall.RawConn) error {
		return conn.Control(func(fd uintptr) {
			if cb := currentCallback(); cb != nil {
				cb.Protect(int(fd))
			}
		})
	}
}

func removeHook() {
	dialer.DefaultSocketHook = nil
}
