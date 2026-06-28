# Multi-Node Xray / Sing-box Proxy Panel — Feature Comparison & Gap Analysis

> Research compiled to guide ProxyPanel (Rust, Hub-Agent) implementation.
> Sources: GitHub READMEs + official docs + schema migrations (fetched 2026-06-28).
> Panels covered: Marzban, Marzneshin, 3X-UI, Xboard (V2board), Hiddify-Manager, V2bX, XrayR.

---

## 0. Executive Summary (TL;DR)

The ecosystem splits into **three architectural families**:

| Family | Representative | Model | Multi-node | Billing |
|---|---|---|---|---|
| **Marzban-family** (Python/React) | Marzban, Marzneshin | Monolithic panel + lightweight node agent (gRPC/REST), inbound-template based, per-user credentials injected into shared inbounds | Yes (Marzban-node / Marznode) | None native |
| **V2board-family** (PHP/Laravel) | V2board, Xboard | Panel-only ("wholesale" backend); nodes are dumb backends (XrayR/V2bX) that pull users & push traffic over HTTP API. Per-protocol server tables + node groups + plans | Yes (external XrayR/V2bX) | First-class (plans, orders, payments, coupons, commissions) |
| **Single-node GUI** | 3X-UI, Hiddify-Manager | All-in-one panel+core on one box; multi-node is bolted on | Limited / bolt-on | Minimal (Hiddify: none; 3X-UI: none) |

**For a Rust Hub-Agent panel**, the most relevant inspirations are:
- **Marzneshin's** decoupled Hub (panel) ↔ Marznode (agent) architecture and the **Service** abstraction (user ↔ inbound access control).
- **Xboard/V2board's** data model: `group` (node sets), `plan`, `order`, `payment`, per-server `rate` (traffic coefficient), `parent_id` (child nodes), and `v2_stat_user` (per-user-per-server-rate traffic accounting).
- **Marzban's** inbound-template JSON model, Host-settings variables, and **on-hold** account lifecycle.
- **3X-UI's** per-client traffic/IP-limit accounting and protocol breadth (incl. WireGuard/Hysteria2/XHTTP/ECH).

A ranked gap analysis appears at the end (§9).

---

## 1. Marzban (Gozargah/Marzban)

- Repo: `github.com/Gozargah/Marzban` — ~7k★, AGPL-3.0, Python 65% / TypeScript 27%. Latest v0.8.4 (Jan 2025). Active.
- Tagline: "Unified GUI Censorship Resistant Solution Powered by Xray."

### 1.1 Architecture
- **Language/Framework**: Python (FastAPI + Uvicorn) backend, React/TypeScript dashboard. DB via SQLAlchemy + Alembic migrations.
- **Database**: SQLite (default), MySQL, MariaDB.
- **Single/Multi-node**: Multi-node via a separate **Marzban-node** process (`gozargah/marzban-node`, Docker). The master panel runs xray-core locally; remote nodes also run xray-core and are controlled from the master.
- **Agent connection**: Node connects to master using **mTLS client certificates** (`ssl_client_cert.pem`). Two service protocols: legacy **RPyC** (default) or **REST** (recommended for v0.4.4+, more stable). Ports: `SERVICE_PORT=62050`, `XRAY_API_PORT=62051`. Master pushes xray config to nodes; nodes expose xray's gRPC API (Stats Service) so the master can read per-user traffic counters. A node can connect to **multiple panels** by running multiple node containers with distinct cert files and ports (host-network or port-mapping modes).

### 1.2 User/Client management
- **User model**: username + password; each user has a `uuid`/`password` used as the xray client credential. Users get a **list of enabled inbounds** (by tag) — i.e. multi-protocol per user.
- **Traffic limits**: `data_limit` (bytes) + `expire_date`. Periodic traffic limit (`data_limit_reset_strategy`: daily, weekly, monthly, etc.) with reset counters.
- **Expiry**: absolute `expire_date`; optional auto-delete of expired/limited users after N days (`USERS_AUTODELETE_DAYS`).
- **IP/device limits**: per-user IP limit (enforced via xray access log + online IP tracking); `online_at` connectivity status.
- **User status**: `active`, `disabled`, `limited` (hit traffic cap), `expired`, and **`on_hold`** (see §8.1).

### 1.3 Node management
- Add node from dashboard: name, address (IP), connection port, API port, **Usage Ratio** (consumption coefficient), and a toggle "Add this node as a new host for every inbound."
- No built-in load balancing or health-check failover in the panel itself; distribution is manual via Host settings (user picks which host to connect to in subscription).

### 1.4 Inbound/Config management
- **Inbounds are raw Xray JSON objects** stored in `xray_config.json` ("Core Settings"). Each inbound is a **template** with `"clients": []` empty. The docs (`xray-inbounds.md`) ship a library of ready templates: VLESS/VMess/Trojan/SS × {TCP, WS, gRPC, H2, XHTTP, HTTPUpgrade, SplitHTTP, KCP} × {TLS, REALITY, None}, plus **Fallback** configs (multiple protocols on one port via xray `fallbacks`).
- **How users attach to inbounds**: the panel injects each user's credential (uuid/password/email) into the `clients` array of every inbound the user is enabled for. Share links/subscription entries are generated per (user × inbound × host).
- **Config push**: master rebuilds xray config and pushes to nodes; nodes reload xray.
- **Multi-inbound per node**: yes; multiple inbounds with unique tags/ports.
- TLS/REALITY supported; REALITY `privateKey` generated via `xray x25519`, `shortId` via `openssl rand -hex 8`. Public key auto-derived.

### 1.5 Subscription system
- One subscription URL per user (token-based path). `XRAY_SUBSCRIPTION_URL_PREFIX` can put subs on a separate domain.
- **Formats**: V2ray base64 (V2RayNG, SingBox, Nekoray, OneClick…), **Clash**, **ClashMeta**. QR codes + share links auto-generated.
- **Custom config injection**: per-client "custom JSON config" toggles (`USE_CUSTOM_JSON_FOR_V2RAYNG/V2RAYN/STREISAND/...`) to inject extra JSON per client app.
- **Host settings** (see §1.6) drive subscription generation: multiple addresses per inbound, variable remakes.
- Customizable subscription page template (`SUBSCRIPTION_PAGE_TEMPLATE`) and Clash template (`CLASH_SUBSCRIPTION_TEMPLATE`).

### 1.6 Host Settings (the "frontend address" layer)
- Per-inbound override of: Remark, Address, Port, SNI, Host, Path, Security, ALPN, Fingerprint. Host settings **override** inbound defaults in generated links (lets user-facing port differ from real inbound port — CDN/frontend pattern).
- **Variables** in Remark/Address: `{SERVER_IP}`, `{USERNAME}`, `{DATA_USAGE}`, `{DATA_LEFT}`, `{DATA_LIMIT}`, `{DAYS_LEFT}`, `{TIME_LEFT}`, `{EXPIRE_DATE}`, `{JALALI_EXPIRE_DATE}`, `{STATUS_EMOJI}`, `{PROTOCOL}`, `{TRANSPORT}`.
- **Random subdomain** via `*` in SNI/Host (`*.example.com` → random subdomain per user; needs wildcard cert). Multiple Host/SNI separated by commas → randomly picked per user.

### 1.7 Traffic accounting
- Per-user `used_traffic` (up+down) accumulated from xray Stats Service gRPC (per inbound/client). Resettable per period.
- Online IPs tracked from xray access log. `NOTIFY_REACHED_USAGE_PERCENT` (default 80%) and `NOTIFY_DAYS_LEFT` (default 3) trigger warnings.
- No native per-node traffic coefficient (the node "Usage Ratio" is a coarse consumption coefficient, not a multiplier on counted bytes the way V2board's `rate` is).

### 1.8 Billing/Payment
- **None native.** No plans/orders/payments. (This is the single biggest gap vs V2board-family.)

### 1.9 Admin dashboard
- Built-in Web UI (React): users table, inbounds/core config editor, hosts, nodes, system stats (CPU/RAM/disk), traffic charts, multi-admin (WIP).

### 1.10 API
- **Full REST API** (FastAPI). Swagger/ReDoc at `/docs` & `/redoc` when `DOCS=True`. JWT auth (`JWT_ACCESS_TOKEN_EXPIRE_MINUTES`, default 1440, 0=infinite). Admin (sudo) vs regular admin roles.

### 1.11 Notifications
- Integrated **Telegram bot** (management + notifications). **Webhook** notifications (POST with `x-webhook-secret` header) for `user_created/updated/deleted/limited/expired/disabled/enabled`. Retry config (`NUMBER_OF_RECURRENT_NOTIFICATIONS`, timeout). No native email/Discord.

### 1.12 Security
- TLS for panel & subscription (`UVICORN_SSL_CERTFILE/KEYFILE`). mTLS for node connections. JWT tokens. Customizable web base path. Recommended behind nginx with a domain (dashboard not exposed by IP for security).

### 1.13 Special features
- **On-hold accounts** (§8.1). **Multi-admin** (WIP). **CLI** (`marzban-cli`) with auto-completion. **Backup service** that zips data and sends to Telegram (scheduled hourly, splits large files). Webhook integration. Fallback configs ("all on one port"). REALITY. Multi-language UI (EN/FA/RU/ZH).

---

## 2. Marzneshin (marzneshin/Marzneshin)

- Repo: `github.com/marzneshin/Marzneshin` — ~695★, AGPL-3.0, TypeScript 66% / Python 27%. A **fork of Marzban** "aiming for scalability." Latest v0.7.4 (Jul 2025).
- Backend: Python (FastAPI); Dashboard: TypeScript/React. Docs: `docs.marzneshin.org`.

### 2.1 Architecture
- **Decoupled from VPN backends**: Marzneshin (the Hub/panel) controls **Marznode** (`marzneshin/marznode`) instances. Marznode runs and manages the actual VPN backends (xray, sing-box, hysteria2). This is the closest analog to ProxyPanel's Hub-Agent split.
- **Multi-core**: xray-core, sing-box, hysteria2 — backends are pluggable per node.
- DB: SQLAlchemy/Alembic (SQLite/Postgres). CLI: `marzneshin-cli`. Resilient/fault-tolerant node management.

### 2.2 Core concepts (from docs `concepts.md`) — the key differentiator
- **Node**: backbone; runs proxy backends; has a set of inbounds.
- **Inbound**: the gateway for users into a node (defined per backend).
- **Host**: connection info (address/SNI/port/path…) so users can *find* an inbound.
- **Service**: **manages which inbounds are accessible to which users.** The first entity you create after install. This is the access-control bridge between Users and Inbounds — cleaner than Marzban's per-user inbound-tag list.

> This **Node → Inbound → Host → Service → User** model is the cleanest abstraction in the ecosystem and maps very well onto ProxyPanel's `ProtocolConfig → NodeBinding → Subscription` flow.

### 2.3 User/Client management
- Data limits + expiry; periodic traffic reset (daily, weekly, …). Multi-user on a single inbound. Per-user access to inbounds is governed by **Services** (a user subscribes to a service which bundles inbounds).
- Multi-admin (WIP, tracked in issue #73).

### 2.4 Node management
- Multi-node for traffic distribution, scalability, fault tolerance. Decoupled backend means a node can run different cores. Resilient node management (reconnects, fault tolerance).

### 2.5 Inbound/Config management
- Inbounds defined per backend (xray / sing-box / hysteria2) on the node; Hub pushes/coordinates. Services gate which users reach which inbounds.

### 2.6 Subscription
- V2ray-compatible (V2RayNG, OneClick, Nekoray…), Clash, ClashMeta. Share links + QR codes. Subscription settings configurable (see `how-to-guides/subscription-settings`).

### 2.7 Traffic accounting
- Per-user data + expiry; periodic reset. Stats for system/nodes/traffic/users.

### 2.8 Billing/Payment
- **None native** (inherits Marzban's non-billing orientation).

### 2.9 API / Admin / Notifications / Security
- **RESTful API**. Web UI dashboard. **Telegram bot** (how-to guide). CLI. Multi-language (EN/FA/RU/Kurdish/Arabic/ZH). Kubernetes/multi-deployment strategies (WIP).

### 2.10 Special features
- **Service abstraction** (decoupled user↔inbound access). **Multi-core** (xray + sing-box + hysteria2). Decoupled backend (panel doesn't run xray directly). Resilient/fault-tolerant node management. Most relevant architectural template for ProxyPanel.

---

## 3. 3X-UI (MHSanaei/3x-ui)

- Repo: `github.com/MHSanaei/3x-ui` — **~41.7k★** (the most popular), GPL-3.0, Go 51% / TypeScript 42%. Latest v3.4.1 (Jun 2026). Very active.
- Go backend, React/TypeScript frontend. Single binary + xray-core. Docs wiki at `docs.sanaei.dev`.

### 3.1 Architecture
- **Single-box all-in-one**: panel + xray-core in one process/binary. DB: **SQLite** (default, `/etc/x-ui/x-ui.db`) or **PostgreSQL** (recommended for high client counts / multi-node; env `XUI_DB_TYPE`/`XUI_DB_DSN`).
- **Multi-node**: supported ("manage and scale across multiple servers from a single panel") but historically bolted-on; the primary model is one panel per server.
- Multi-arch (amd64/386/arm64/armv7/armv6/armv5/s390x) and broad OS support. Docker image bundles **Fail2ban** for IP-limit enforcement (`NET_ADMIN` cap required).

### 3.2 User/Client management
- **Per-client** (not per-user-account) management inside inbounds: each client has **traffic quota**, **expiry date**, **IP limit**, **live online status**, one-click share/QR/subscription.
- **Traffic stats per inbound, per client, and per outbound**, with reset controls.
- IP limits enforced via **Fail2ban** (bans offenders with iptables).

### 3.3 Inbound/Config management
- **Protocol breadth is the headline feature**: VLESS, VMess, Trojan, Shadowsocks, **WireGuard**, **Hysteria2**, HTTP, SOCKS (Mixed), Dokodemo-door/Tunnel, TUN.
- Transports: TCP (Raw), mKCP, WebSocket, gRPC, HTTPUpgrade, **XHTTP**. Security: TLS, XTLS, **REALITY**, **ECH**, post-quantum tagged.
- **Fallbacks** (multiple protocols on one port, e.g. VLESS+Trojan on 443).
- Outbound & routing: **WARP**, **NordVPN**, custom routing rules, load balancers, outbound proxy chaining.
- Custom subscription page templates (`docs/custom-subscription-templates.md`).

### 3.4 Subscription
- Built-in subscription server with **multiple output formats** + custom page templates.

### 3.5 Traffic accounting
- Per inbound / per client / per outbound, with reset controls. Tunnel health monitor (`XUI_TUNNEL_HEALTH_*` envs) probes a URL through the tunnel and restarts xray after repeated failures (note: restart drops clients).

### 3.6 Billing/Payment
- **None.** Explicitly "intended for personal use only."

### 3.7 API / Admin / Notifications / Security
- **RESTful API** with in-panel Swagger. **Telegram bot** for remote monitoring/management. 13 UI languages, dark/light themes. Configurable web base path. Fail2ban. Tunnel health monitor.

### 3.8 Special features
- Widest protocol/transport support in the list. ECH, XHTTP, post-quantum tags. WARP/NordVPN outbound presets. Terraform provider (`terraform-provider-3x-ui`) for IaC. Unattended/cloud-init install (`XUI_NONINTERACTIVE`). DB migration SQLite→PostgreSQL tooling. Best-in-class single-node feature surface.

---

## 4. Xboard / V2board (cedar2025/Xboard, v2board/v2board)

- Xboard: `github.com/cedar2025/Xboard` — ~4.5k★, MIT, **PHP 94%** (Laravel 12 + Octane). Active fork of V2board (v2board/v2board, ~5k★, last release 1.7.4 Jun 2023 — stalled). Xboard is the maintained successor.
- Stack: Laravel 12 + Octane (Swoole/RoadRunner) for performance; Admin = React + Shadcn UI; User frontend = Vue3 + TS + NaiveUI; Redis cache; Docker. DB: MySQL 5.7+ (and SQLite supported for quick start).
- This is the **"wholesale / commercial"** family: panel-only, with plans/orders/payments/coupons/commissions, and **external node backends** (XrayR / V2bX) that talk to the panel API.

### 4.1 Architecture (panel↔node split — critical for ProxyPanel)
- **The panel does NOT run xray.** It is a pure management/billing/subscription layer.
- **Node backends** (XrayR or V2bX) are deployed on each proxy server. They:
  1. **Pull** the user list (and their `uuid`/credentials, traffic limits, IP/device limits) from the panel's HTTP API (authenticated by per-server token).
  2. Run xray/sing-box locally with those users as xray clients.
  3. **Push back** per-user up/down traffic counters to the panel periodically (the panel then aggregates and enforces limits).
- This is a **poll/report HTTP API** model (not a long-lived gRPC stream like Marzban-node). ProxyPanel's Hub-Agent gRPC stream is a more modern equivalent.

### 4.2 Data model (reconstructed from `database/migrations/*`)

**Users (`v2_user`)** — `id, email, password, balance, commission_type/rate/balance, u (upload bytes), d (download bytes), transfer_enable (quota), banned, is_admin, is_staff, uuid, group_id, plan_id, speed_limit, token, expired_at, remind_expire, remind_traffic`. Later migrations add: **`device_limit`** (2025_01_10), **traffic-reset fields + `next_reset_at`** (2025_06_21 / 2026_04_19 backfill).

**Plans (`v2_plan`)** — `group_id, transfer_enable, name, speed_limit, show, renew, content, month_price, quarter_price, half_year_price, year_price, two_year_price, three_year_price, onetime_price, reset_price, reset_traffic_method, capacity_limit, tags` (tags added 2025_07_01). `reset_traffic_method`: `null`=follow system, `0`=1st of month, `1`=monthly, `2`=never, `3`=Jan 1 yearly, `4`=yearly.

**Node groups (`v2_server_group`)** — a named group of servers. **A user's `group_id` (from their plan) determines which set of nodes they can subscribe to.** This is the "user subscribes to a set of nodes" concept.

**Server routing (`v2_server_route`)** — `match`/`action`/`action_value` routing rules applied on nodes.

**Per-protocol server tables** (original V2board) — `v2_server_vmess`, `v2_server_vless`, `v2_server_trojan`, `v2_server_shadowsocks`, `v2_server_hysteria`. Each has:
- `group_id`, `route_id`, `parent_id` (**parent node — child/relay concept**), `tags`, `name`, **`rate` (traffic coefficient/multiplier)**, `host` (user-facing address), `port` (user-facing port), `server_port` (real backend port), `tls`, `tls_settings`, `network`, `network_settings`, `flow`, `show`, `sort`.
- Xboard later unifies these into **`v2_server_table`** (2025_01_05), then adds `custom_config` + cert (2026_03_15) and traffic fields (2026_03_28), and **machine support / `machine_load_history`** (2026_04_11 / 2026_04_18) = node health/load monitoring.

**Traffic stats:**
- `v2_stat_server`: `server_id, server_type, u, d, record_type('d'|'m'), record_at` — per-server daily/monthly.
- `v2_stat_user`: `user_id, server_rate, u, d, record_type, record_at` — **per-user-per-server-rate** accounting. This is exactly how "traffic coefficient per node" is implemented: counted bytes = real bytes × server `rate`, and stats are bucketed by rate so reports stay accurate.

**Commerce:**
- `v2_order` — `plan_id, coupon_id, payment_id, type(1 new/2 renew/3 upgrade), period, trade_no, total_amount, status, commission_status, paid_at, surplus/refund/balance amounts`.
- `v2_payment` — gateway integration: `payment` (driver name), `config` (JSON), `notify_domain`, `handling_fee_fixed/percent`.
- `v2_coupon` — `code, type, value, limit_use, limit_use_with_user, limit_plan_ids, limit_period, started_at, ended_at`.
- `v2_commission_log` + `v2_invite_code` — **referral/affiliate** system (commission_type: system/period/onetime).
- `v2_ticket` + `v2_ticket_message` — **support tickets**.
- `v2_knowledge` — knowledge base / FAQ.
- `v2_notice` — announcements.
- `v2_mail_log` — email log.
- `v2_subscribe_templates` (2025_07_27) — customizable subscription templates.
- `gift_card` tables (2025_07_01) — gift cards.
- `admin_audit_log` (2026_03_11) — admin audit logging (replaces `v2_log`).

### 4.3 User/Client management
- Email-based accounts with `balance` (credit), `transfer_enable` quota, `u`/`d` counters, `expired_at`, `speed_limit`, `device_limit`, `banned`, `group_id` (from plan), `plan_id`. Reset cycle per plan or per system. Capacity limits per plan (`capacity_limit`).

### 4.4 Node management
- Servers belong to **groups**; users get nodes via their plan's group. `parent_id` enables parent/child (relay) node topology. `rate` per node = traffic multiplier. `tags` for filtering. `show`/`sort` for UI ordering. **Machine load history** (added 2026) = health/performance monitoring.

### 4.5 Inbound/Config management
- Server = inbound definition (per protocol originally, unified later). `host`/`port` vs `server_port` decouples user-facing address from real port (CDN/frontend). `custom_config` + cert per server (2026_03_15). Configs are pushed by the node backend pulling from the panel API (the panel stores the inbound spec; XrayR/V2bX renders it into xray config).

### 4.6 Subscription
- Token-based subscription URL per user (`token` column). Generates links for **all nodes in the user's group/plan** — this is the multi-node subscription. Formats: V2ray base64, Clash, ClashMeta, Sing-box (driven by `v2_subscribe_templates`). QR codes.

### 4.7 Traffic accounting
- Node backends report `u`/`d` per user back to the panel; panel updates `v2_user.u/d` and writes `v2_stat_user` (bucketed by `server_rate`) and `v2_stat_server` (per server). Daily/monthly aggregation via `record_type`. Traffic counted = real × `rate`.

### 4.8 Billing/Payment — **first-class**
- Plans with recurring + onetime pricing (month/quarter/half-year/year/two-year/three-year/onetime), `reset_price` (buy more traffic). Orders (new/renew/upgrade). Payment gateways (pluggable, with handling fees). Coupons (limits per user/plan/period). Referral commissions (system/period/onetime). Balance/credit. Gift cards. Surplus/refund accounting on plan changes.

### 4.9 Admin dashboard
- React + Shadcn admin (users, plans, orders, servers, payments, coupons, notices, tickets, knowledge, settings, stats). Vue3 user frontend (subscriptions, orders, tickets, knowledge, invite).

### 4.10 API
- REST (Laravel). Admin & user roles. Personal access tokens (`personal_access_tokens`). User token for subscription.

### 4.11 Notifications
- Email (`v2_mail_log`, mail templates added 2026_04_20). Telegram (channel). Tickets for user↔admin messaging.

### 4.12 Security
- Laravel auth, personal access tokens, admin path customizable (restart required), audit log. Rate limiting via Laravel. TLS via reverse proxy.

### 4.13 Special features
- **Full commerce suite** (plans/orders/payments/coupons/commissions/gift cards). **Node groups** (user subscribes to a set of nodes). **Per-node `rate` traffic coefficient** with per-user-per-rate stats. **Parent/child nodes**. **Traffic reset methods** (5 modes). **Device limit**. **Speed limit**. **Capacity limit** per plan. **Support tickets**, **knowledge base**, **announcements**. **Admin audit log**. **Machine/node load monitoring**. **Plugin system** (`plugins`/`plugins-core`, plugin dev guide). Maintainable architecture (React admin, Vue user). Note: project is in "light maintenance" mode (critical fixes only, limited new features).

---

## 5. Hiddify-Manager (hiddify/Hiddify-Manager)

- Repo: `github.com/hiddify/Hiddify-Manager` — ~9.1k★, GPL-3.0, **Python 48% / Shell 26% / Jinja 13%**. Latest v12.3.3 (May 2026). Very active, optimized for censorship circumvention in China/Russia/Iran.
- Multi-core: **Xray + SingBox** (+ Telegram proxy). Listed by Xray.

### 5.1 Architecture
- All-in-one installer-driven panel (`hiddify-panel`) + orchestration via shell/Ansible-style configs + nginx + HAProxy + acme.sh. Heavy use of **Jinja templates** to render xray/sing-box configs. Docker-based.
- Not a clean Hub/Agent split like Marzneshin; node distribution is via the Hiddify ecosystem (Hiddify-Node), but the dominant model is a single manager box.

### 5.2 User/Client management
- Multi-user with **time and traffic limits per user**. **Multiple admin privileges**. Dedicated per-user pages to view consumption and configs. Telegram-bot-based user management.

### 5.3 Node management
- Multiple core support. **Auto CDN IP** configuration. **Multiple domains**. Automatic Cloudflare connection. Auto update + auto backup (every 6h). (Multi-node distribution exists but the README emphasizes single-box robustness.)

### 5.4 Inbound/Config management
- **20+ protocols** across Direct/CDN/Domain-Fronting matrices:
  - Reality (VLESS XTLS/gRPC), SSH, Hysteria2, TUICv5, WireGuard.
  - Trojan / VLESS / VMess over {WS, TCP, gRPC, H2, XTLS, HTTP} × {TLS, HTTP} + "Fake" variants for fronting.
  - Shadowsocks + ShadowTLS (TLS/HTTP/H2/H3).
- Smart proxy modes (1: only filtered sites; 2: all except domestic; 3: all). Resistant to detection. **DNS over HTTPS** (CDN-supported). **Redirector** (CDN-supported) to hand off SS/Telegram URLs to client apps. Dedicated WARP.

### 5.5 Subscription
- "Dedicated and smart configurations"; dedicated client software (Hiddify-Next). Smart proxy for domestic/filtered sites. Per-user config pages.

### 5.6 Traffic accounting
- Per-user time + traffic limits; consumption shown on user pages.

### 5.7 Billing/Payment
- **None.** Free/anti-censorship focused.

### 5.8 API / Admin / Notifications / Security
- Web panel + Telegram bot. Multiple admin privileges. DoH. Auto-backup. Optimized to disable all ports except 22/80/443 to reduce detection.

### 5.9 Special features
- **Widest protocol matrix** (esp. ShadowTLS, TUIC, Hysteria2, SSH, Telegram proxy). **Smart proxy routing** modes. **DoH server**. **Redirector** for client handoff. **Auto CDN IP**, multi-domain, auto-Cloudflare. **Auto backup/update**. Jinja-templated config rendering. Strong censorship-evasion focus (China/Russia/Iran). Only SingBox panel with user management (per their claim).

---

## 6. V2bX (Yxvd/V2bX — and related forks)

> **Note:** The referenced repo `github.com/Yxvd/V2bX` returned HTTP 404 at fetch time (the project appears to have been moved/removed). The well-known fork `github.com/wyx268/V2bX` also 404'd. The description below is from the role V2bX plays in the V2board/Xboard ecosystem (the node-side backend) and its general public documentation; treat specifics as needing re-verification.

- **Role**: Go-based **node-side backend** for V2board/Xboard panels — the successor to **XrayR**. It runs on each proxy server, syncs users from the panel, runs xray/sing-box, and reports traffic back.
- **Languages**: Go. Supports Shadowsocks, VMess, VLESS, Trojan, Hysteria(2), TUIC, WireGuard (via xray/sing-box cores).
- **How it connects**: configured with the panel API URL + a per-server token; periodically **pulls users** (with credentials, `rate`, traffic limits, IP/device limits) and **pushes traffic** (per-user `u`/`d`) to the panel's report endpoints. This is the implementation behind Xboard's `v2_stat_user`/`v2_user.u/d` updates.
- **Config**: per-inbound definitions (matched to the panel's server tables); supports multiple inbounds/nodes; traffic accounting with the panel's `rate` multiplier semantics.
- **Why it matters for ProxyPanel**: V2bX/XrayR is the reference for the **Agent side** of a V2board-compatible system. ProxyPanel's `pp-agent` occupies the same role but uses a stateful **gRPC bidirectional stream** (Hub pushes configs, Agent reports traffic) instead of V2bX's poll/report HTTP pattern — lower latency, easier config-push, and naturally supports real-time online-IP / health telemetry.

---

## 7. XrayR (XrayR-project/XrayR)

- Repo: `github.com/XrayR-project/XrayR` — ~2.9k★. **Deprecated ("项目已废弃").** The original Go node-side backend for V2board/SSPanel. Predecessor of V2bX.
- Same role as V2bX: pull users from panel API, run xray, report traffic. Supported V2board & SSPanel-v3 panel APIs. Now superseded by V2bX; mentioned here for historical/architectural context only — **do not target compatibility with XrayR; target the V2board/Xboard API surface** if panel-side interop is desired.

---

## 8. Special Topics (explicitly requested)

### 8.1 Marzban "On Hold" accounts (usage cooldown)
Marzban's user model has a `status` field with values including `active`, `disabled`, `limited`, `expired`, and **`on_hold`**. The on-hold lifecycle (a "usage cooldown" / "first-connect activation") works as follows:

- An admin creates a user flagged **on hold** with two extra fields:
  - **`on_hold_expire_duration`**: the subscription duration (e.g. 30 days) that will be granted once the account activates.
  - **`on_hold_timeout`**: an absolute deadline by which the user must make their first connection (after this, the hold slot may be released/expired).
- While on hold, **the expiry clock does NOT run** and (by config) traffic accounting may be deferred — the account is provisioned (credential registered in xray) but its "subscription timer" is paused.
- On the user's **first actual connection** (detected via xray access log / online status), Marzban flips status → `active`, sets `expire_date = first_connect_time + on_hold_expire_duration`, and starts normal traffic/expiry accounting.
- Use case: selling accounts that "activate on first use" so the buyer's paid period doesn't start ticking until they actually connect — fairer than setting a fixed expiry at purchase.

> ProxyPanel implementation guidance: add a `ClientStatus` enum with `OnHold`, plus `on_hold_expire_duration` and `on_hold_timeout` on the client entity. The Hub should watch Agent "first online" events and transition `OnHold → Active`, computing `expire_date` lazily. This is a high-value, differentiating feature carried by almost no panel except Marzban.

### 8.2 Marzban/Marzneshin inbound "templates" and user↔inbound attachment
- **Marzban templates**: each inbound is a **raw Xray JSON object** (the "template") with an empty `"clients": []`. Templates live in the master's `xray_config.json` (editable via Core Settings UI). The docs provide a full library (VLESS/VMess/Trojan/SS × transport × security + Fallback). A user is attached to an inbound by **injecting the user's credential** (uuid/password/email) into that inbound's `clients` array on every node the user should reach. Each user carries a list of enabled inbound **tags**. Share links & subscription entries are produced per (user × inbound × host). **Host Settings** then layer user-facing address/port/SNI/path overrides on top, with `{VARIABLE}` placeholders and random-subdomain `*` support.
- **Marzneshin model**: explicit entities — **Node** (runs backends) → **Inbound** (gateway) → **Host** (connection info) → **Service** (access control: which users can use which inbounds). A **Service** bundles a set of inbounds; users are granted services. This is the cleaner, normalized version of Marzban's implicit per-user-tag-list, and it maps directly onto ProxyPanel's `ProtocolConfig → NodeBinding → (user binds to service/group)` design.

### 8.3 V2board/Xboard node model (server_table + traffic reporting)
- **Server representation**: originally one table per protocol (`v2_server_vmess/vless/trojan/shadowsocks/hysteria`), unified in Xboard into `v2_server_table` (2025-01-05). Columns capture: `group_id`, `route_id`, `parent_id` (relay/child), `tags`, `name`, **`rate`** (traffic coefficient), `host` (user-facing address) + `port` (user-facing) vs `server_port` (real), `tls`/`tls_settings`, `network`/`network_settings`, `flow`, `show`, `sort`; later `custom_config` + cert (2026-03-15), traffic fields (2026-03-28), and **machine/load history** (2026-04-11/18).
- **Node↔panel protocol**: nodes (XrayR/V2bX) hold a per-server token and (1) **pull** users + inbound specs + limits from the panel, (2) **push** per-user `u`/`d` counters back. The panel writes `v2_user.u/d` (running totals) and `v2_stat_user` (`user_id, server_rate, u, d, record_type, record_at`) — bucketing by `server_rate` so the per-node `rate` multiplier is accurately reflected in reports — plus `v2_stat_server` for per-node daily/monthly aggregates.
- **Node grouping & user subscription set**: `v2_server_group` groups servers; a `Plan` references a `group_id`; a `User` has a `group_id` (from their plan). **A user's subscription therefore contains all servers in their group** — this is the "user subscribes to a set of nodes" mechanism. `parent_id` enables hierarchical/relay topologies; `route_id` ties in `v2_server_route` routing rules.

### 8.4 Subscription URL with multiple nodes
- All panels converge on: **one token-based subscription URL per user** that expands to a list of configs for the nodes/inbounds the user is entitled to.
  - **Marzban/Marzneshin**: subscription = all inbounds the user is enabled for (via tags / via Services) × all Hosts defined on those inbounds (Hosts carry per-node address/port/SNI, so each Host effectively yields a node entry). Adding a node = adding a Host to inbounds (or toggling "add node as host for every inbound").
  - **Xboard/V2board**: subscription = all servers in the user's `group_id` (= their plan's group); each server row yields one config (with `host`/`port`/`rate`/`tls`/`network`). `v2_subscribe_templates` customize output.
  - **3X-UI**: per-client subscription containing the inbounds that client is attached to.
- Output formats across the ecosystem: **V2ray base64**, **Clash / ClashMeta (YAML)**, **sing-box JSON**, **V2RayNG/V2RayN custom JSON**, plus QR codes and share links.

---

## 9. Feature Gap Analysis — ranked recommendations for ProxyPanel

Ranked by **impact × ecosystem prevalence**, with implementation notes tied to ProxyPanel's existing crates (`pp-hub`, `pp-agent`, `pp-db`, `pp-config`, `pp-subscription`, `pp-common`, `pp-proto`, `pp-web`). Each item flags which panel(s) validate the feature.

### Tier 1 — Must-have (table stakes across all serious panels)

1. **Per-client traffic quota + expiry + periodic reset cycles**
   - Ubiquitous (Marzban, Marzneshin, 3X-UI, Xboard, Hiddify). Xboard's 5 `reset_traffic_method` modes (monthly 1st, by-month, never, yearly Jan-1, by-year) + per-plan override + `next_reset_at` scheduler are the gold standard.
   - *Impl*: `Client { data_limit, used_up, used_down, expire_date, reset_strategy, reset_cycle_days, next_reset_at }`; Hub cron resets `used_*` and updates `next_reset_at`.

2. **Multi-node Hub↔Agent with config push + traffic report (already in ProxyPanel design)**
   - Core to Marzban-node/Marznode and V2board/XrayR. ProxyPanel's gRPC bidi-stream is *better* than V2bX's HTTP poll — keep it. Ensure: Hub pushes `ConfigPush` on any inbound/client change; Agent reports per-client `u/d` + online IPs + health on interval and on-event.
   - *Impl*: extend `HubMessage::ConfigPush` + `AgentMessage::TrafficReport` (already planned); add `AgentMessage::OnlineIps` and `AgentMessage::Health`.

3. **Inbound "template" model with multi-user on a shared inbound**
   - Marzban's raw-Xray-JSON-template + empty `clients[]` + per-user credential injection is the proven pattern. ProxyPanel's `ProtocolConfig` + `BuilderRegistry` already abstracts this; ensure a `ProtocolConfig` can be bound to many nodes (`NodeBinding`) and many clients, and that the builder injects each client's uuid/email/password.
   - *Impl*: confirm `pp-config` builders inject a `Vec<ClientCredential>` into the inbound `clients`/`users` array for xray *and* sing-box.

4. **Protocol breadth: VLESS/VMess/Trojan/Shadowsocks + REALITY + TLS + WS/gRPC/XHTTP/HTTPUpgrade + Fallbacks**
   - Marzban & 3X-UI set the bar. 3X-UI adds WireGuard/Hysteria2/XHTTP/ECH. At minimum implement the Marzban template library; add Hysteria2/TUIC for sing-box nodes (Marzneshin/Hiddify do).
   - *Impl*: `pp-common::ProtocolType` + `pp-config::{xray,singbox}` builders; `validate_protocol` rules in hub routes.

5. **Subscription system with multiple formats + QR + share links**
   - Everyone has this. Required formats: **V2ray base64**, **Clash/ClashMeta YAML**, **sing-box JSON**, **V2RayNG/V2RayN custom JSON**. ProxyPanel already scaffolds this in `pp-subscription/formats`.
   - *Impl*: ensure `generate_subscription(SubscriptionFormat)` covers all four; add per-format custom-config injection hooks (Marzban's `USE_CUSTOM_JSON_FOR_*`).

6. **Token-based subscription URL that expands to the user's full node set**
   - Universal. Driven by user↔inbound/host bindings (Marzban) or group/plan (Xboard).
   - *Impl*: `Subscription { token }` → resolve Client → active `NodeBinding`s → inject credentials → render per format. (Already in ProxyPanel's subscription flow design.)

7. **REST API + JWT auth + admin/user roles + OpenAPI docs**
   - Marzban (FastAPI/Swagger), 3X-UI (Swagger), Xboard (Laravel). ProxyPanel's axum HTTP API should expose Swagger/reDoc and JWT with role scopes.
   - *Impl*: `pp-hub/routes/*` + a `SecurityConfig` with role-scoped JWT; enable `utoipa`/OpenAPI.

8. **Per-client IP limit + online-IP tracking (enforcement)**
   - Marzban (log-based), 3X-UI (Fail2ban). ProxyPanel should track online IPs via Agent access-log events and disconnect/ban offenders (xray block by `block`/stats, or fail2ban-style on the node).
   - *Impl*: Agent streams online IPs; Hub enforces per-client `ip_limit`; Agent applies xray `block` rules or iptables on overflow.

### Tier 2 — High-value differentiators (present in the leading panels)

9. **On-hold / first-connect-activation accounts** ⭐
   - Only Marzban has this. High user-facing value for paid accounts. §8.1.
   - *Impl*: `ClientStatus::OnHold` + `on_hold_expire_duration` + `on_hold_timeout`; transition on first Agent "online" event.

10. **Node groups + plan→group → user subscribes to a set of nodes**
    - Xboard's `v2_server_group`/`Plan.group_id`/`User.group_id` is the cleanest. ProxyPanel can model `NodeGroup` + `Plan` + assign clients to groups; subscription resolves all nodes in the client's group.
    - *Impl*: `NodeGroup` entity; `Plan.group_id`; `Client.group_id`; subscription resolver filters by group.

11. **Per-node traffic coefficient (`rate`) with per-user-per-rate stats**
    - Xboard's `rate` + `v2_stat_user(server_rate, u, d, …)`. Essential for priced traffic (e.g. expensive exits cost more).
    - *Impl*: `NodeBinding { rate }` (or `Node.rate`); traffic counter stores counted = real × rate; stats bucketed by (client, node, rate, period).

12. **Host/Host-settings layer (user-facing address ≠ real port) + variable remakes**
    - Marzban Host Settings with `{USERNAME}/{DAYS_LEFT}/{DATA_LEFT}/…` and random `*.domain` subdomains. Enables CDN fronting and per-user cosmetic remakes.
    - *Impl*: `Host { inbound_id, remark, address, port, sni, host, path, security, alpn, fingerprint }` with template variable substitution in `pp-subscription`.

13. **Node health monitoring + auto-reconnect/resilient agent**
    - Marzneshin "resilient/fault-tolerant node management"; 3X-UI tunnel health monitor; Xboard `machine_load_history`. ProxyPanel's Agent already has exponential-backoff reconnect (AGENTS.md §4.2). Add Hub-side health dashboard + Agent CPU/mem/network/load telemetry + tunnel-health probe like 3X-UI.
    - *Impl*: `AgentMessage::Health { cpu, mem, net_in, net_out, load }`; Hub stores `node_health_history`; optional tunnel-probe with restart-on-failure (cooldown).

14. **Parent/child (relay) node topology**
    - Xboard `parent_id`. Useful for relay/CDN chains.
    - *Impl*: `Node.parent_id` (self-FK); subscription renders parent-chain addresses when set.

15. **Telegram bot + Webhook notifications + email**
    - Marzban (Telegram + Webhook), 3X-UI (Telegram), Xboard (email + tickets). ProxyPanel should provide a notification service with pluggable channels (Telegram, email, webhook) and event types (`user_created/updated/deleted/limited/expired/disabled/enabled`), with retry (Marzban's `NUMBER_OF_RECURRENT_NOTIFICATIONS`/timeout).
    - *Impl*: `pp-hub/service/notify` with a `Notifier` trait and channel impls; webhook secret header.

16. **CLI for admin operations**
    - Marzban `marzban-cli`, Marzneshin `marzneshin-cli`. ProxyPanel already has a `proxy-panel` CLI (`cargo run --bin proxy-panel -- init-db`). Extend with admin/user/node management subcommands.
    - *Impl*: extend `pp-cli` with `admin create`, `user {create,list,reset}`, `node {list,register}`.

### Tier 3 — Commerce & platform (decide based on target audience)

17. **Plans + Orders + Payments + Coupons + Commissions (full billing)** — Xboard only
    - Only V2board-family has this fully. If ProxyPanel targets commercial use, this is a major build: `Plan` (recurring + onetime pricing, reset_price, capacity_limit), `Order` (new/renew/upgrade), `Payment` (pluggable gateways + handling fees), `Coupon`, `Commission`/invite, `balance`/credit, gift cards, surplus/refund accounting. Xboard's schema is the reference blueprint.
    - *Impl*: big new domain in `pp-db` + `pp-hub/routes` + `pp-web` user frontend. Consider phasing: Plans/Orders/Balance first, then Payment gateways, then Coupons/Commissions.

18. **Support tickets + Knowledge base + Announcements + Admin audit log** — Xboard
    - `v2_ticket`/`v2_ticket_message`, `v2_knowledge`, `v2_notice`, `admin_audit_log`. Standard SaaS amenities.
    - *Impl*: ticket & knowledge entities + routes; audit-log middleware on admin mutations.

19. **Plugin system** — Xboard (`plugins`/`plugins-core`)
    - Allows extending payment/notice/etc without forking. Nice-to-have; defer unless ecosystem wanted.

### Tier 4 — Niche / censorship-evasion differentiators (Hiddify/3X-UI)

20. **Smart-proxy routing modes (filtered-only / all-except-domestic / all) + geo routing rules**
    - Hiddify; 3X-UI (Iran/Russia v2ray rules). For censorship-focused deployments.
    - *Impl*: routing-rule sets attached to subscription generation (`v2_server_route` analog); `pp-config` rule injection.

21. **Auto CDN IP / multi-domain / auto-Cloudflare / DoH server / Redirector**
    - Hiddify-specific. Defer unless targeting the Iran/China/Russia use case.

22. **Widest protocol matrix: Hysteria2 / TUIC / WireGuard / ShadowTLS / SSH / Telegram proxy / ECH**
    - 3X-UI + Hiddify. Implement on demand via sing-box builder; prioritize Hysteria2 + TUIC + WireGuard.

23. **Backup service (scheduled, to Telegram/s3)** — Marzban
    - *Impl*: Hub cron → dump DB + configs → upload to Telegram/S3 with chunking.

24. **IaC / unattended install / cloud-init** — 3X-UI
    - Nice for deployment; ProxyPanel already has Docker/deploy assets.

---

## 10. Cross-cutting implementation notes for ProxyPanel

- **Architecture alignment**: ProxyPanel's Hub-Agent gRPC stream is superior to both Marzban's REST/RPyC node protocol and V2bX's HTTP poll. Keep bidi-stream; add event types: `ConfigPush`, `TrafficReport`, `OnlineIps`, `Health`, `NodeCommand` (restart/reload), `OnHoldActivate`.
- **Data model priorities** (extend `pp-db/entities`):
  - `Client` + status enum (`Active/Disabled/Limited/Expired/OnHold`), `on_hold_*`, `device_limit`, `ip_limit`, `speed_limit`, `reset_strategy/next_reset_at`.
  - `Node` + `parent_id`, `rate`, `tags`, `health` fields.
  - `NodeGroup` + `Plan.group_id` + `Client.group_id` (Tier 2 #10) — or a `Service`-style abstraction (Marzneshin) linking clients to inbound sets.
  - `Host` table for the user-facing-address layer (Tier 2 #12).
  - `NodeBinding { node_id, protocol_config_id, rate }` (already in design) — ensure `rate` lives here or on `Node`.
  - Stats: `stat_user(client_id, node_id, rate, u, d, record_type, record_at)` + `stat_server(node_id, u, d, record_type, record_at)` (Xboard pattern).
  - Commerce (Tier 3, if pursued): `plan`, `order`, `payment`, `coupon`, `commission_log`, `ticket`, `knowledge`, `notice`, `audit_log`.
- **Subscription** (`pp-subscription`): ensure the resolver merges `ProtocolConfig` + `Host` overrides + client credentials, then fans out per format. Add variable substitution (`{USERNAME}`, `{DATA_LEFT}`, `{DAYS_LEFT}`, `{PROTOCOL}`, `{TRANSPORT}`) and random-`*` SNI/Host.
- **Config builders** (`pp-config`): maintain a template library equivalent to Marzban's `xray-inbounds.md` (VLESS/VMess/Trojan/SS × transport × security + Fallback), plus sing-box equivalents (incl. Hysteria2/TUIC) — make adding a new protocol a single `build_inbound` impl (AGENTS.md §5.5).
- **Security**: JWT role scopes (admin/user), per-agent mTLS or per-node token (already effectively mTLS via gRPC + cert), admin audit log, rate limiting on panel API, customizable admin path.

---

*End of report. Use §9 ranking to prioritize the ProxyPanel roadmap; §8 for precise semantics of on-hold, inbound templates, node model, and multi-node subscriptions; §4.2 schema as a concrete blueprint for entities.*
