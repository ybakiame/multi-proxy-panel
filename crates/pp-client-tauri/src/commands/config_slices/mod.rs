//! Config slices (ADR-0005) Tauri commands.
//!
//! Frontend-facing commands for the structured DNS / custom outbound slices
//! stored at `data_dir/config_slices.json`. The full [`ConfigSlices`] document
//! is itself the serde wire contract (tagged/flattened outbound protocols), so
//! there is no separate View/Input split here; validation happens inside
//! [`ConfigSlicesStore::save`] and a validation failure never touches the file.

use pp_client::config_slices::{ConfigSlices, ConfigSlicesStore};
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Pure command bodies (unit-testable without a Tauri runtime)
// ---------------------------------------------------------------------------

/// Load the full config slices document from `data_dir`.
///
/// A missing or corrupted `config_slices.json` yields [`ConfigSlices::default`]
/// (all slices disabled), matching [`ConfigSlicesStore::load`] (ADR-0005 §3.6).
pub(crate) fn load_slices(data_dir: &std::path::Path) -> Result<ConfigSlices, String> {
    let store = ConfigSlicesStore::new(data_dir.to_path_buf());
    store
        .load()
        .map_err(|e| format!("failed to load config slices: {e}"))
}

/// Validate then save the full config slices document (frontend full-patch).
///
/// [`ConfigSlicesStore::save`] validates before writing: on failure the error is
/// forwarded verbatim and the file is left untouched (ADR-0005 §3.5/§3.6).
pub(crate) fn save_slices(data_dir: &std::path::Path, input: ConfigSlices) -> Result<(), String> {
    let store = ConfigSlicesStore::new(data_dir.to_path_buf());
    store
        .save(&input)
        .map_err(|e| format!("failed to save config slices: {e}"))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Get the full config slices document (serialized `ConfigSlices`).
#[tauri::command]
pub fn config_slices_get(state: State<'_, AppState>) -> Result<ConfigSlices, String> {
    load_slices(&state.data_dir)
}

/// Save the full config slices document (frontend full-patch).
#[tauri::command]
pub fn config_slices_save(state: State<'_, AppState>, input: ConfigSlices) -> Result<(), String> {
    save_slices(&state.data_dir, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_client::config_slices::*;

    fn sample_slices() -> ConfigSlices {
        ConfigSlices {
            version: SLICE_VERSION,
            dns: DnsSlice::default(),
            outbounds: OutboundsSlice {
                items: vec![CustomOutbound {
                    id: "o1".to_string(),
                    name: "Node One".to_string(),
                    enabled: true,
                    protocol: OutboundProtocol::Vless(VlessOutbound {
                        server: "example.com".to_string(),
                        server_port: 443,
                        uuid: "uuid-1".to_string(),
                        flow: "xtls-rprx-vision".to_string(),
                        ..Default::default()
                    }),
                    builtin: false,
                }],
            },
            experimental: ExperimentalSlice::default(),
            route: RouteSlice::default(),
        }
    }

    #[test]
    fn get_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let slices = load_slices(dir.path()).unwrap();
        assert_eq!(slices, ConfigSlices::default());
        assert!(
            !ConfigSlicesStore::new(dir.path().to_path_buf())
                .slices_file()
                .exists()
        );
    }

    #[test]
    fn save_then_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        save_slices(dir.path(), sample_slices()).unwrap();
        let loaded = load_slices(dir.path()).unwrap();
        assert_eq!(loaded, sample_slices());
    }

    #[test]
    fn save_invalid_does_not_write_and_forwards_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut slices = sample_slices();
        slices.dns.mode = DnsMode::Takeover;
        slices.dns.final_tag = "dangling".to_string();

        let err = save_slices(dir.path(), slices).unwrap_err();
        assert!(
            err.contains("validation") || err.contains("dns.final"),
            "{err}"
        );
        assert!(
            !ConfigSlicesStore::new(dir.path().to_path_buf())
                .slices_file()
                .exists()
        );
    }

    /// The frontend TS types are generated from this serde shape: the tagged
    /// `OutboundProtocol` is flattened into `CustomOutbound`, so `type` sits
    /// next to `id`/`name`/`enabled` and protocol fields.
    #[test]
    fn custom_outbound_serializes_flattened_tagged_protocol() {
        let value = serde_json::to_value(sample_slices()).unwrap();
        let item = &value["outbounds"]["items"][0];
        assert_eq!(item["type"], "vless");
        assert_eq!(item["server"], "example.com");
        assert_eq!(item["server_port"], 443);
        assert_eq!(item["uuid"], "uuid-1");
        assert_eq!(item["flow"], "xtls-rprx-vision");
        assert!(item["tls"].is_object());
        assert!(item["transport"].is_object());

        // Round-trip through JSON mirrors what the Tauri command argument does.
        let back: ConfigSlices = serde_json::from_value(value).unwrap();
        assert_eq!(back, sample_slices());
    }
}
