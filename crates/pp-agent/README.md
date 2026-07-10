# ProxyPanel Agent

部署在代理节点上的代理程序，负责管理本地 xray/sing-box 进程，并通过 gRPC 长连接与 Hub 通信。

## 构建

```bash
# Debug
cargo build --bin proxy-panel-agent

# Release
cargo build --release --bin proxy-panel-agent
```

产物：`target/release/proxy-panel-agent`

## 运行

```bash
./target/release/proxy-panel-agent \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token" \
  --data-dir /var/lib/proxy-panel/agent \
  --bin-dir /usr/local/bin
```

## 参数说明

| 参数 | 说明 |
|------|------|
| `--hub-url` | Hub gRPC 地址，`http://` 或 `https://` |
| `--token` | 节点注册 Token（由 Hub 生成） |
| `--data-dir` | 运行时数据目录 |
| `--bin-dir` | xray/sing-box 二进制所在目录 |
| `--tls-ca` | 自定义 CA 证书路径（用于自签名 TLS） |
| `--tls-domain` | TLS 证书域名 |

## 环境变量

```bash
PROXYPANEL_HUB_URL=https://grpc.example.com:50052
PROXYPANEL_AGENT_TOKEN=your-agent-token
```

## 获取 Agent Token

在 Hub 侧创建节点后，使用 CLI 生成或查看 Token：

```bash
# 生成新 Token（不写入数据库）
cargo run --bin proxy-panel -- agent-token --node-name "tokyo-01"

# 或在 Hub 创建节点时自动输出 Token
```

## 测试

```bash
cargo test -p pp-agent
```

## 部署

生产部署请使用 systemd 服务文件，参见 [deploy/README.md](../deploy/README.md) 和 [docs/deployment.md](../docs/deployment.md)。
