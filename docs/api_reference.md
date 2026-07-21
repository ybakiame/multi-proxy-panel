# ProxyPanel API 参考文档

本文档描述 ProxyPanel Hub 提供的全部 REST API 端点。

**基础 URL:** `http://localhost:8081`（默认）

**内容类型:** 所有请求和响应均为 `application/json`，除非另有说明。

---

## 目录

1. [通用约定](#通用约定)
2. [健康检查](#健康检查)
3. [节点管理](#节点管理)
4. [协议配置](#协议配置)
5. [节点绑定](#节点绑定)
6. [客户端管理](#客户端管理)
7. [订阅管理](#订阅管理)
8. [流量查询](#流量查询)
9. [指标查询](#指标查询)
10. [日志查询](#日志查询)
11. [公开订阅端点](#公开订阅端点)
12. [错误响应](#错误响应)

---

## 通用约定

### 认证

> **注意:** 当前版本使用预留的认证中间件。生产部署建议通过反向代理添加认证层。

### 请求格式

- `POST` / `PUT` 请求体为 JSON
- `GET` 查询参数使用标准 URL 编码
- UUID 使用标准字符串格式（如 `550e8400-e29b-41d4-a716-446655440000`）

### 响应格式

成功响应统一包装：

```json
{
  "data": { ... }
}
```

列表响应包含分页信息：

```json
{
  "data": [ ... ],
  "total": 100
}
```

---

## 健康检查

### GET `/health`

检查 Hub 服务状态。

**响应:**

```json
{
  "status": "ok"
}
```

**状态码:**
- `200 OK` — 服务正常

---

## 节点管理

### GET `/api/v1/nodes`

获取所有节点列表。

**响应:**

```json
{
  "data": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Tokyo-01",
      "hostname": "tokyo-01.example.com",
      "address": "1.2.3.4",
      "cores_available": ["xray", "sing-box"],
      "labels": { "region": "ap-northeast", "tier": "premium" },
      "status": "online",
      "last_seen_at": "2024-01-15T08:30:00Z"
    }
  ]
}
```

**状态码:**
- `200 OK`
- `500 Internal Server Error`

---

### GET `/api/v1/nodes/{id}`

获取单个节点详情。

**路径参数:**
- `id` — 节点 UUID

**响应:**

```json
{
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Tokyo-01",
    "hostname": "tokyo-01.example.com",
    "address": "1.2.3.4",
    "status": "online"
  }
}
```

**状态码:**
- `200 OK`
- `404 Not Found` — 节点不存在
- `500 Internal Server Error`

---

### POST `/api/v1/nodes`

创建新节点。

**请求体:**

```json
{
  "name": "Tokyo-01",
  "hostname": "tokyo-01.example.com",
  "address": "1.2.3.4"
}
```

**字段说明:**

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 节点显示名称 |
| `hostname` | string | ❌ | 主机名 |
| `address` | string | ❌ | IP 地址或域名 |

**响应:**

```json
{
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Tokyo-01",
    "hostname": "tokyo-01.example.com",
    "address": "1.2.3.4",
    "status": "connecting"
  }
}
```

**状态码:**
- `201 Created`
- `400 Bad Request` — 缺少必需字段
- `500 Internal Server Error`

---

### DELETE `/api/v1/nodes/{id}`

删除节点。

**路径参数:**
- `id` — 节点 UUID

**状态码:**
- `204 No Content`
- `404 Not Found`
- `500 Internal Server Error`

---

### POST `/api/v1/nodes/{id}/push`

向指定节点推送配置。

**路径参数:**
- `id` — 节点 UUID

**请求体:**

```json
{
  "core_type": "sing-box",
  "restart": true
}
```

**字段说明:**

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `core_type` | string | ❌ | `xray` / `sing-box` / `mihomo`（默认 `sing-box`） |
| `restart` | boolean | ❌ | 是否重启核心（默认 `true`） |
| `version` | string | ❌ | 配置版本号；缺省时 Hub 使用配置内容的 SHA-256 哈希（前 16 位十六进制）。Agent 对版本一致的重复推送会跳过应用，避免重连后不必要的核心重启 |

**响应:**

```json
{
  "success": true,
  "message": "config pushed"
}
```

**状态码:**
- `200 OK`
- `400 Bad Request`
- `404 Not Found` — 节点不存在
- `502 Bad Gateway` — Agent 未连接
- `500 Internal Server Error`

---

## 协议配置

### GET `/api/v1/protocols`

获取协议配置列表（支持分页）。

**查询参数:**

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `page` | integer | 1 | 页码 |
| `per_page` | integer | 20 | 每页数量（最大 100） |

**响应:**

```json
{
  "data": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440001",
      "name": "vless-reality-443",
      "protocol_type": "vless_reality",
      "core_type": "xray",
      "listen_port": 443,
      "listen_address": "0.0.0.0",
      "settings": { "clients": [], "decryption": "none" },
      "tls_settings": { "reality": { "show": false } }
    }
  ],
  "total": 42
}
```

---

### GET `/api/v1/protocols/{id}`

获取单个协议配置详情。

**路径参数:**
- `id` — 配置 UUID

---

### POST `/api/v1/protocols`

创建协议配置。

**请求体:**

```json
{
  "name": "vless-reality-443",
  "protocol_type": "vless_reality",
  "core_type": "xray",
  "listen_port": 443,
  "listen_address": "0.0.0.0",
  "settings": {
    "clients": [],
    "decryption": "none"
  },
  "tls_settings": {
    "reality": {
      "show": false,
      "dest": "www.example.com:443",
      "serverNames": ["www.example.com"]
    }
  }
}
```

**字段说明:**

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 配置名称 |
| `protocol_type` | string | ❌ | 协议类型（默认 `vless_reality`） |
| `core_type` | string | ❌ | 目标核心 `xray` / `sing-box` / `mihomo`（默认 `xray`） |
| `listen_port` | integer | ❌ | 监听端口（默认 `443`） |
| `listen_address` | string | ❌ | 监听地址（默认 `0.0.0.0`） |
| `settings` | object | ❌ | 协议特定配置 |
| `tls_settings` | object | ❌ | TLS / REALITY 配置 |

**兼容性矩阵:**

| 协议 | 允许的核心 |
|------|-----------|
| `vless_reality` | `xray`, `sing-box`, `mihomo` |
| `vless_xhttp` | `xray`, `mihomo` |
| `hysteria2` | `sing-box`, `mihomo` |
| `anytls` | `sing-box`, `mihomo` |

> TLS 采用分层模型：协议配置只声明 `{"enabled": true}` 是否启用 TLS；具体证书在**节点绑定**的 `override_settings.tls_settings` 中复写——`{"cert_id": "..."}`（托管证书，需属于绑定节点）、`{"certFile": "...", "keyFile": "..."}`（显式证书文件）、`{"domain": "..."}`（内置 ACME，**仅 sing-box 协议**）。订阅链接的 SNI 自动取证书域名。

**状态码:**
- `201 Created`
- `400 Bad Request` — 协议与核心不兼容
- `500 Internal Server Error`

---

### PUT `/api/v1/protocols/{id}`

更新协议配置。

**路径参数:**
- `id` — 配置 UUID

**请求体:** 与 POST 相同，所有字段可选。

---

### DELETE `/api/v1/protocols/{id}`

删除协议配置。

---

### GET `/api/v1/utils/generate-reality-keys`

生成 X25519 密钥对用于 REALITY 配置。

**响应:**

```json
{
  "data": {
    "private_key": "AABC...xyz=",
    "public_key": "DEFG...123=",
    "short_id": "a1b2c3d4"
  }
}
```

---

## 核心版本

核心版本目录由用户从上游版本中选择保存（release / prerelease），协议配置可通过 `core_version` 引用，Agent 按需安装对应版本的核心二进制。

### GET `/api/v1/core-versions`

列出核心版本记录。

**查询参数:**
- `core_type` (可选) — 按核心过滤：`xray` / `sing-box` / `mihomo`

**响应:**

```json
{
  "data": {
    "versions": [
      {
        "id": "uuid",
        "core_type": "mihomo",
        "version": "v1.19.29",
        "channel": "release",
        "created_at": "2026-07-20T00:00:00Z"
      }
    ]
  }
}
```

### GET `/api/v1/core-versions/upstream`

从 GitHub Releases 拉取上游版本（只读，不入库），每条记录标注是否已保存。每核心每渠道最多返回 10 个。

**查询参数:**
- `core_type` (可选) — 只拉取指定核心

**响应:**

```json
{
  "data": {
    "cores": [
      {
        "core_type": "mihomo",
        "versions": [
          { "version": "v1.19.29", "channel": "release", "saved": true },
          { "version": "Prerelease-Alpha", "channel": "prerelease", "saved": false }
        ]
      }
    ]
  }
}
```

### POST `/api/v1/core-versions`

保存用户选中的版本（已存在的记录跳过，幂等）。

**请求体:**

```json
{
  "versions": [
    { "core_type": "mihomo", "version": "v1.19.29", "channel": "release" }
  ]
}
```

**响应:**

```json
{
  "data": { "added": 1 }
}
```

### DELETE `/api/v1/core-versions/{id}`

删除一条版本记录（不影响已引用它的协议配置）。

**状态码:**
- `204 No Content`
- `404 Not Found`

---

## 证书管理

托管证书由节点 Agent 内置 ACME 客户端（Let's Encrypt，HTTP-01 挑战）签发，统一存放于节点数据目录 `certs/<domain>.{crt,key}`，三个核心的 TLS 配置均可引用。协议配置的 `tls_settings` 使用 `{"cert_id": "..."}` 关联（要求证书属于绑定节点且状态为有效）。

### GET `/api/v1/certificates`

列出证书记录。

**查询参数:**
- `node_id` (可选) — 按节点过滤

**响应:**

```json
{
  "data": {
    "certificates": [
      {
        "id": "uuid",
        "node_id": "uuid",
        "node_name": "东京-01",
        "domain": "hy2.example.com",
        "status": "active",
        "challenge_type": "http-01",
        "expires_at": "2026-10-18T00:00:00Z",
        "last_issued_at": "2026-07-20T00:00:00Z",
        "last_error": null,
        "created_at": "2026-07-20T00:00:00Z"
      }
    ]
  }
}
```

`status` 取值：`pending`（签发中）/ `active`（有效）/ `failed`（失败，见 `last_error`）。

### POST `/api/v1/certificates`

创建证书记录并向节点 Agent 下发签发请求。域名需已解析到该节点，签发期间节点 80 端口需可被 Let's Encrypt 访问。Agent 离线时记录保持 `pending`，注册后自动补发。

**请求体:**

```json
{
  "domain": "hy2.example.com",
  "node_id": "uuid",
  "challenge_type": "http-01"
}
```

### POST `/api/v1/certificates/{id}/renew`

手动触发续期（Agent 也会在证书满 60 天时自动续期，Let's Encrypt 证书有效期 90 天）。

### DELETE `/api/v1/certificates/{id}`

删除证书记录（节点上已签发的文件不删除）。

**状态码:**
- `204 No Content`
- `404 Not Found`

---

## 节点绑定

### GET `/api/v1/bindings`

获取所有节点-配置绑定关系。

**响应:**

```json
{
  "data": [
    {
      "id": "770e8400-e29b-41d4-a716-446655440002",
      "node_id": "550e8400-e29b-41d4-a716-446655440000",
      "protocol_config_id": "660e8400-e29b-41d4-a716-446655440001",
      "override_settings": null,
      "is_active": true,
      "created_at": "2024-01-15T10:00:00Z"
    }
  ]
}
```

---

### POST `/api/v1/bindings`

创建绑定关系。

**请求体:**

```json
{
  "node_id": "550e8400-e29b-41d4-a716-446655440000",
  "protocol_config_id": "660e8400-e29b-41d4-a716-446655440001",
  "override_settings": null,
  "is_active": true
}
```

---

### DELETE `/api/v1/bindings/{id}`

删除绑定关系。

---

## 客户端管理

### GET `/api/v1/clients`

获取所有客户端列表。

**响应:**

```json
{
  "data": [
    {
      "id": "880e8400-e29b-41d4-a716-446655440003",
      "user_id": "990e8400-e29b-41d4-a716-446655440004",
      "name": "user001",
      "email": "user001@example.com",
      "traffic_limit_bytes": 1099511627776,
      "traffic_used_bytes": 104857600,
      "expiry_date": "2024-12-31T23:59:59Z",
      "reset_day": 1,
      "status": "active"
    }
  ]
}
```

**状态说明:**

| 状态 | 含义 |
|------|------|
| `active` | 正常 |
| `disabled` | 手动禁用 |
| `limited` | 流量超限 |
| `expired` | 已过期 |
| `on_hold` | 暂停 |

---

### GET `/api/v1/clients/{id}`

获取单个客户端详情。

---

### POST `/api/v1/clients`

创建客户端。

**请求体:**

```json
{
  "name": "user001",
  "email": "user001@example.com",
  "traffic_limit_bytes": 1099511627776,
  "reset_day": 1
}
```

---

### PUT `/api/v1/clients/{id}`

更新客户端信息。

---

### DELETE `/api/v1/clients/{id}`

删除客户端。

---

## 订阅管理

### GET `/api/v1/templates`

获取订阅模板列表。

**响应:**

```json
{
  "data": [
    {
      "id": "aa0e8400-e29b-41d4-a716-446655440005",
      "name": "default-base64",
      "format": "base64",
      "base_config": null,
      "filter_rules": null
    }
  ]
}
```

---

### POST `/api/v1/templates`

创建订阅模板。

**请求体:**

```json
{
  "name": "clash-premium",
  "format": "clash",
  "base_config": {
    "port": 7890,
    "socks-port": 7891
  },
  "filter_rules": {
    "include_regions": ["ap-northeast"]
  }
}
```

**格式选项:** `base64`, `json`, `clash`, `sing-box`, `v2rayng`

---

### DELETE `/api/v1/templates/{id}`

删除订阅模板。

---

### GET `/api/v1/subscriptions`

获取订阅记录列表。

**响应:**

```json
{
  "data": [
    {
      "id": "bb0e8400-e29b-41d4-a716-446655440006",
      "client_id": "880e8400-e29b-41d4-a716-446655440003",
      "template_id": "aa0e8400-e29b-41d4-a716-446655440005",
      "token": "aBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890",
      "url_path": "/sub/aBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890",
      "is_active": true
    }
  ]
}
```

---

### POST `/api/v1/subscriptions`

创建订阅。

**请求体:**

```json
{
  "client_id": "880e8400-e29b-41d4-a716-446655440003",
  "template_id": "aa0e8400-e29b-41d4-a716-446655440005"
}
```

**响应:**

```json
{
  "data": {
    "id": "bb0e8400-e29b-41d4-a716-446655440006",
    "token": "aBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890",
    "url_path": "/sub/aBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890"
  }
}
```

---

### DELETE `/api/v1/subscriptions/{id}`

删除订阅。

---

## 流量查询

### GET `/api/v1/traffic`

查询节点入站级流量统计记录（按小时聚合），按 `hour_bucket` 倒序返回。

**查询参数:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `node_id` | UUID | 按节点筛选 |
| `client_id` | UUID | 按客户端筛选 |
| `start` | RFC 3339 | 起始时间（含），如 `2024-01-15T00:00:00Z` |
| `end` | RFC 3339 | 结束时间（含） |
| `limit` | integer | 最大返回数量（默认 500，上限 5000） |

**响应:**

```json
{
  "data": [
    {
      "id": "bb0e8400-e29b-41d4-a716-446655440006",
      "node_id": "550e8400-e29b-41d4-a716-446655440000",
      "protocol_config_id": null,
      "client_id": null,
      "hour_bucket": "2024-01-15T08:00:00Z",
      "upload_bytes": 1073741824,
      "download_bytes": 2147483648
    }
  ]
}
```

---

### GET `/api/v1/usage`

查询用户级用量记录（`node_user_usage_records`，按小时聚合），按 `hour_bucket` 倒序返回。

**查询参数:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `node_id` | UUID | 按节点筛选 |
| `client_id` | UUID | 按客户端筛选 |
| `start` | RFC 3339 | 起始时间（含） |
| `end` | RFC 3339 | 结束时间（含） |
| `limit` | integer | 最大返回数量（默认 500） |

**响应:**

```json
{
  "data": [
    {
      "id": "cc0e8400-e29b-41d4-a716-446655440007",
      "node_id": "550e8400-e29b-41d4-a716-446655440000",
      "client_id": "880e8400-e29b-41d4-a716-446655440003",
      "hour_bucket": "2024-01-15T08:00:00Z",
      "upload_bytes": 104857600,
      "download_bytes": 524288000,
      "rate": 1.0
    }
  ]
}
```

---

### GET `/api/v1/usage/summary`

按客户端或节点聚合用量，按总流量倒序返回。

**查询参数:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_by` | string | 聚合维度：`client`（默认）或 `node` |
| `node_id` | UUID | 按节点筛选 |
| `client_id` | UUID | 按客户端筛选 |
| `start` | RFC 3339 | 起始时间（含） |
| `end` | RFC 3339 | 结束时间（含） |
| `limit` | integer | 最大返回数量（默认 20） |

**响应:**

```json
{
  "data": [
    {
      "id": "880e8400-e29b-41d4-a716-446655440003",
      "upload_bytes": 104857600,
      "download_bytes": 524288000,
      "total_bytes": 629145600
    }
  ]
}
```

其中 `id` 为客户端 ID（`group_by=client`）或节点 ID（`group_by=node`）。

**错误响应:**
- `400 Bad Request` — `group_by` 不是 `client` 或 `node`

---

## 指标查询

### GET `/api/v1/metrics`

查询主机指标记录。

**查询参数:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `node_id` | UUID | 按节点筛选 |
| `from` | ISO 8601 | 起始时间 |
| `to` | ISO 8601 | 结束时间 |
| `limit` | integer | 最大返回数量 |

**响应:**

```json
{
  "data": [
    {
      "node_id": "550e8400-e29b-41d4-a716-446655440000",
      "timestamp": "2024-01-15T08:30:00Z",
      "cpu_percent": 15.5,
      "mem_used": 8589934592,
      "mem_total": 17179869184,
      "net_rx": 104857600,
      "net_tx": 52428800,
      "load_avg1": 0.5
    }
  ]
}
```

---

### GET `/api/v1/metrics/{node_id}/latest`

获取指定节点的最新指标。

---

## 日志查询

### GET `/api/v1/logs`

查询系统日志。

**查询参数:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `level` | string | 日志级别: `error`, `warn`, `info`, `debug` |
| `source` | string | 日志来源筛选 |
| `limit` | integer | 最大返回数量（默认 100） |

**响应:**

```json
{
  "data": [
    {
      "id": "cc0e8400-e29b-41d4-a716-446655440007",
      "level": "info",
      "source": "agent-550e8400-e29b-41d4-a716-446655440000",
      "message": "config applied for Xray",
      "metadata": { "target": "pp_agent::client" },
      "created_at": "2024-01-15T08:30:00Z"
    }
  ]
}
```

---

## 公开订阅端点

### GET `/sub/{token}`

通过订阅 Token 获取客户端配置。这是面向最终用户的公开端点，无需认证。

**路径参数:**
- `token` — 订阅令牌（创建订阅时自动生成）

**查询参数:**

| 参数 | 类型 | 说明 |
|------|------|------|
| `format` | string | 覆盖输出格式 |

**响应内容类型:**

| 格式 | Content-Type |
|------|-------------|
| `base64` | `text/plain; charset=utf-8` |
| `json` | `application/json` |
| `clash` | `application/x-yaml` |
| `sing-box` | `application/json` |
| `v2rayng` | `application/json` |

**状态码:**
- `200 OK` — 返回订阅内容
- `404 Not Found` — Token 不存在
- `403 Forbidden` — 订阅已禁用

---

## 错误响应

所有错误响应统一格式：

```json
{
  "error": {
    "code": "internal_error",
    "message": "详细错误信息"
  }
}
```

### HTTP 状态码对照

| 状态码 | 场景 |
|--------|------|
| `400 Bad Request` | 请求参数错误、格式不兼容 |
| `401 Unauthorized` | 认证失败（预留） |
| `403 Forbidden` | 权限不足、订阅已禁用 |
| `404 Not Found` | 资源不存在 |
| `409 Conflict` | 资源冲突（预留） |
| `422 Unprocessable Entity` | 业务逻辑验证失败（预留） |
| `500 Internal Server Error` | 服务器内部错误 |
| `502 Bad Gateway` | Agent 未连接或通信失败 |
| `503 Service Unavailable` | 服务暂时不可用（预留） |

---

## 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| v0.1.0 | 2024-01 | 初始 API 版本 |
