package mihomocore

import (
	"errors"
	"runtime"
	"runtime/debug"
	"strings"
	"sync"

	"github.com/metacubex/mihomo/common/observable"
	"github.com/metacubex/mihomo/config"
	"github.com/metacubex/mihomo/constant"
	"github.com/metacubex/mihomo/dns"
	"github.com/metacubex/mihomo/hub"
	"github.com/metacubex/mihomo/hub/executor"
	LC "github.com/metacubex/mihomo/listener/config"
	"github.com/metacubex/mihomo/log"
	"github.com/metacubex/mihomo/tunnel"
)

var (
	stateMux   sync.Mutex
	callback   Callback
	isSetup    bool
	isRunning  bool
	lastConfig *config.Config
	logSub     observable.Subscription[log.Event]
)

// Setup 设置 mihomo HomeDir、保存回调、解析并应用配置。
//
// TUN 不通过配置启用（由 StartTun 以 VpnService 提供的 fd 创建），
// 外部控制器 HTTP API 在 Android 上默认关闭。
func Setup(homeDir string, configYAML []byte, cb Callback) error {
	stateMux.Lock()
	defer stateMux.Unlock()

	if homeDir == "" {
		return errors.New("mihomocore: homeDir is required")
	}
	if len(configYAML) == 0 {
		return errors.New("mihomocore: configYAML is empty")
	}
	if cb == nil {
		return errors.New("mihomocore: callback is nil")
	}

	constant.SetHomeDir(homeDir)
	callback = cb
	startLogForwarder(cb)

	cfg, err := executor.ParseWithBytes(configYAML)
	if err != nil {
		log.Errorln("[mihomocore] parse config error: %v", err)
		return err
	}

	// 避免 mihomo 自行创建 tun 设备，TUN 由 StartTun(fd, ...) 管理。
	cfg.General.Tun.Enable = false
	cfg.Controller.ExternalController = ""
	cfg.Controller.ExternalControllerTLS = ""
	cfg.Controller.ExternalControllerUnix = ""
	cfg.Controller.ExternalControllerPipe = ""

	hub.ApplyConfig(cfg)

	lastConfig = cfg
	isSetup = true
	isRunning = true
	log.Infoln("[mihomocore] setup done, homeDir=%s", homeDir)
	return nil
}

// Stop 停止 tun listener 与 mihomo（幂等）。
func Stop() {
	stateMux.Lock()
	defer stateMux.Unlock()

	stopLogForwarder()
	StopTun()

	if lastConfig != nil {
		// 零化端口以关闭所有 inbounds，然后重放配置。
		g := lastConfig.General
		g.Port = 0
		g.SocksPort = 0
		g.RedirPort = 0
		g.TProxyPort = 0
		g.MixedPort = 0
		g.ShadowSocksConfig = ""
		g.VmessConfig = ""
		g.TuicServer = LC.TuicServer{}
		g.Tun.Enable = false
		lastConfig.Controller.ExternalController = ""
		lastConfig.Controller.ExternalControllerTLS = ""
		lastConfig.Controller.ExternalControllerUnix = ""
		lastConfig.Controller.ExternalControllerPipe = ""
		hub.ApplyConfig(lastConfig)
		lastConfig = nil
	}

	executor.Shutdown()
	callback = nil
	isSetup = false
	isRunning = false
	log.Infoln("[mihomocore] stopped")
}

// IsRunning 返回核心是否处于运行状态。
func IsRunning() bool {
	stateMux.Lock()
	defer stateMux.Unlock()
	return isRunning
}

// Suspend 暂停/恢复核心流量处理。
func Suspend(suspended bool) {
	stateMux.Lock()
	defer stateMux.Unlock()
	if suspended {
		tunnel.OnSuspend()
	} else {
		tunnel.OnRunning()
	}
}

// UpdateDns 更新系统 DNS 并刷新缓存（对齐 FlClash handleUpdateDns）。
func UpdateDns(dnsList string) {
	go func() {
		dns.UpdateSystemDNS(strings.Split(dnsList, ","))
		dns.FlushCacheWithDefaultResolver()
	}()
}

// ForceGC 请求一次强制 GC。
func ForceGC() {
	log.Infoln("[mihomocore] request force GC")
	runtime.GC()
	debug.FreeOSMemory()
}

// SetCallback 替换回调并重启日志转发。
func SetCallback(cb Callback) {
	stateMux.Lock()
	defer stateMux.Unlock()
	callback = cb
	if isSetup {
		startLogForwarder(cb)
	}
}

func startLogForwarder(cb Callback) {
	if logSub != nil {
		log.UnSubscribe(logSub)
		logSub = nil
	}
	if cb == nil {
		return
	}
	sub := log.Subscribe()
	logSub = sub
	go func() {
		for ev := range sub {
			if cb == nil {
				continue
			}
			cb.WriteLog(int(ev.LogLevel), ev.Payload)
		}
	}()
}

func stopLogForwarder() {
	if logSub != nil {
		log.UnSubscribe(logSub)
		logSub = nil
	}
}

func currentCallback() Callback {
	stateMux.Lock()
	defer stateMux.Unlock()
	return callback
}
