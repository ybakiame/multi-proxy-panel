# ADR-0001: Neutral Fields for Multi-Kernel Protocol Configuration

- **Status:** Accepted
- **Date:** 2026-05-30
- **Deciders:** ProxyPanel Contributors

## Context

ProxyPanel supports both **xray-core** and **sing-box** as proxy cores. The same logical protocol (e.g., VLESS + REALITY) requires completely different JSON field names and structures in each core:

- **xray:** `streamSettings.security = "reality"`, `realitySettings.privateKey`, `realitySettings.shortIds`
- **sing-box:** `tls.reality.enabled = true`, `tls.reality.private_key`, `tls.reality.short_id`

Previously, the frontend stored xray-specific field names directly in `settings` JSON. This caused the sing-box builder to read wrong keys (e.g., `privateKey` instead of `private_key`), producing invalid configurations.

Additionally:
- Hysteria2 and AnyTLS use `users` arrays in sing-box but were incorrectly using `clients` arrays.
- The `anytls` protocol existed in the frontend UI but had no backend enum or builder implementation.
- `tuic` vs `tuic_v5` naming mismatched between frontend and backend parsers.

## Decision

Adopt a **neutral fields** strategy:

1. **Frontend stores kernel-neutral field names** in `protocol_configs.settings` JSON.
   - Example: `reality_dest`, `reality_private_key`, `reality_short_id`, `xhttp_path`, `xhttp_mode`
2. **Backend `ConfigBuilder` implementations** convert neutral fields to kernel-specific formats.
   - `XrayConfigBuilder` → xray JSON field names (`privateKey`, `shortIds`, `dest`)
   - `SingBoxConfigBuilder` → sing-box JSON field names (`private_key`, `short_id`, `handshake.server`)
3. **Backward compatibility:** Builders accept both new neutral names and old legacy names via `or_else` fallback chains.
4. **User credentials are stored uniformly as a `clients` array** in `settings`.
   - `inject_client_credentials()` injects `{id, email, flow}` for UUID-based protocols and `{name, password}` for password-based protocols.
   - Each builder converts `clients` to the target kernel's expected format (`settings.clients` for xray, `users` for sing-box).

## Consequences

### Positive
- A single frontend form works for both kernels; no need to switch fields when changing `core_type`.
- Backend builders are the single source of truth for kernel-specific configuration syntax.
- Adding a new kernel in the future only requires a new `ConfigBuilder` implementation, not frontend changes.
- Existing data remains readable due to fallback logic.

### Negative
- Builders contain more field-mapping logic (fallible at runtime rather than compile-time).
- Subtle differences between kernels (e.g., sing-box `short_id` accepts array, xray `shortIds` accepts array) must be carefully handled in each builder.
- Developers must remember to update **both** builders when adding a new protocol field.

## Alternatives Considered

### Alternative A: Kernel-Specific Fields in Frontend
Store entirely separate field sets per kernel and switch the form dynamically based on `core_type` selection.

- **Rejected:** Doubles frontend complexity. Switching kernels would require field migration logic. Violates DRY.

### Alternative B: Strongly-Typed Settings Structs per Protocol
Define a Rust struct for each protocol's settings in `pp-common`, shared between frontend (via WASM) and backend.

- **Rejected:** High refactoring cost. The project currently uses loose `serde_json::Value` for flexibility. Would require WASM-bindgen exposure and frontend restructuring.

### Alternative C: Store Kernel-Native JSON Directly
Store raw xray JSON in `settings` when `core_type = xray`, and raw sing-box JSON when `core_type = sing-box`.

- **Rejected:** Prevents switching cores without re-entering all fields. Makes subscription generation (which needs a normalized view) much harder.

## Related Files

- `crates/pp-config/src/builder.rs` — `ConfigBuilder` trait definition
- `crates/pp-config/src/xray.rs` — xray builder with neutral-field conversion
- `crates/pp-config/src/singbox.rs` — sing-box builder with neutral-field conversion
- `crates/pp-hub/src/routes/subscription.rs` — `inject_client_credentials()`
- `crates/pp-web/src/pages/protocols.rs` — frontend forms using neutral field names
