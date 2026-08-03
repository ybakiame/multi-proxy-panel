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
// TUN 不通过配置启用（由 StartTun 以 VpnService 提供的 fd 创建）。
// 外部控制器按配置保留：合成配置仅在用户开启 Clash 面板时写入
// `external-controller: 127.0.0.1:port`（见 pp-client core_config.rs
// apply_mihomo_panel_features），Clash 面板 API 与规则模式热切换
// （push_clash_mode）依赖该监听。external-controller 仅监听 127.0.0.1
// 回环地址，不接受外部连接。TLS / Unix / Pipe 变体本应用不用，统一清空。
//
// 锁粒度：Setup 全程持有 stateMux（含 hub.ApplyConfig），期间并发调用
// Stop 会阻塞等待；剥离 external-ui 后 ApplyConfig 不再同步下载面板，
// 该阻塞窗口已消失。
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
	// 保留 ExternalController：配置里的 external-controller 仅在用户开启
	// Clash 面板时由合成配置注入（127.0.0.1 回环，安全）；本应用不使用
	// TLS / Unix / Pipe 变体，统一清空。
	cfg.Controller.ExternalControllerTLS = ""
	cfg.Controller.ExternalControllerUnix = ""
	cfg.Controller.ExternalControllerPipe = ""

	// 无条件剥离 external-ui：external-ui-url 会在 ApplyConfig 路径
	// （updateUpdater → AutoDownloadUI）同步下载面板 zip，首次经代理/
	// 无代理时阻塞 setup 数十秒至超时；本应用自带 UI，无需 mihomo 面板。
	cfg.Controller.ExternalUI = ""
	cfg.Controller.ExternalUIURL = ""
	cfg.Controller.ExternalUIName = ""

	hub.ApplyConfig(cfg)

	lastConfig = cfg
	isSetup = true
	isRunning = true
	log.Infoln("[mihomocore] setup done, homeDir=%s", homeDir)
	return nil
}

// Stop 停止 tun listener 与 mihomo（幂等）。
//
// 锁粒度：Stop 全程持有 stateMux；Setup 持锁期间调用 Stop 会阻塞等待
// （剥离 external-ui 后 Setup 不再有网络下载的长时间窗口，该阻塞已消失）。
func Stop() {
	stateMux.Lock()
	defer stateMux.Unlock()

	// 全程 recover 防御：任何 Go panic 只记日志、不 abort 进程
	// （gomobile 下 panic = SIGABRT = App 闪退，Kotlin try/catch 接不住）。
	defer func() {
		if r := recover(); r != nil {
			log.Errorln("[mihomocore] stop panic: %v", r)
		}
	}()

	stopLogForwarder()
	StopTun()

	if lastConfig != nil {
		// 零化端口以关闭所有 inbounds，然后重放配置。
		// 必须保留：executor.Shutdown() 只关闭 tun listener
		// （listener.Cleanup），并不关闭 HTTP/SOCKS/Redir/TProxy/Mixed
		// 等配置 inbounds；零化重放中的 ReCreateXXX(0)/PatchInboundListeners
		// 才是关闭它们的路径。运行期中重放会重新初始化组件，已由上方
		// recover 兜底。
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
