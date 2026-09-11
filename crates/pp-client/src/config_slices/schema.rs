//! Config slice container + cross-slice validation (ADR-0005 §3.1/§3.5).

use std::collections::HashSet;

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};

use super::outbound::outbound_tag;
use super::{DnsMode, DnsSlice, OutboundsSlice};

/// Current schema version (stored in [`ConfigSlices::version`]).
pub const SLICE_VERSION: u32 = 1;

/// serde default for [`ConfigSlices::version`].
#[must_use]
pub const fn default_slice_version() -> u32 {
    SLICE_VERSION
}

/// Config slice container, stored at `data_dir/config_slices.json`.
///
/// All fields are `#[serde(default)]`: an old client without this file loads
/// [`ConfigSlices::default`] (every slice disabled).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSlices {
    /// Schema version for future migrations (currently [`SLICE_VERSION`]).
    #[serde(default = "default_slice_version")]
    pub version: u32,
    #[serde(default)]
    pub dns: DnsSlice,
    #[serde(default)]
    pub outbounds: OutboundsSlice,
}

impl Default for ConfigSlices {
    fn default() -> Self {
        Self {
            version: SLICE_VERSION,
            dns: DnsSlice::default(),
            outbounds: OutboundsSlice::default(),
        }
    }
}

impl ConfigSlices {
    /// Validate the whole slice set (ADR-0005 §3.5).
    ///
    /// Checks tag uniqueness, reference integrity and address/port validity.
    /// Called by [`crate::config_slices::ConfigSlicesStore::save`] before writing.
    pub fn validate(&self) -> PanelResult<()> {
        self.dns.validate()?;
        self.outbounds.validate()?;
        Ok(())
    }
}

impl DnsSlice {
    /// Validate DNS tags, references and addresses.
    pub fn validate(&self) -> PanelResult<()> {
        let mut tags: HashSet<&str> = HashSet::new();
        for server in &self.servers {
            if server.tag.trim().is_empty() {
                return Err(validation("DNS server tag must not be empty"));
            }
            if server.tag.chars().any(char::is_whitespace) {
                return Err(validation(format!(
                    "DNS server tag `{}` must not contain whitespace",
                    server.tag
                )));
            }
            if !tags.insert(server.tag.as_str()) {
                return Err(validation(format!(
                    "duplicate DNS server tag `{}`",
                    server.tag
                )));
            }
            if server.server_type.uses_server() && server.server.trim().is_empty() {
                return Err(validation(format!(
                    "DNS server `{}` requires a server address",
                    server.tag
                )));
            }
            if server.server_port == Some(0) {
                return Err(validation(format!(
                    "DNS server `{}` has invalid port 0",
                    server.tag
                )));
            }
        }

        if !self.final_tag.is_empty() && !tags.contains(self.final_tag.as_str()) {
            return Err(validation(format!(
                "dns.final `{}` does not reference a defined DNS server",
                self.final_tag
            )));
        }
        if self.enabled && matches!(self.mode, DnsMode::Takeover) && self.final_tag.is_empty() {
            return Err(validation(
                "dns.final is required when DNS takeover is enabled",
            ));
        }

        for rule in self.rules.iter().filter(|r| r.enabled) {
            if rule.server_tag.trim().is_empty() {
                return Err(validation(format!(
                    "DNS rule `{}` has an empty server tag",
                    rule.id
                )));
            }
            if !tags.contains(rule.server_tag.as_str()) {
                return Err(validation(format!(
                    "DNS rule `{}` references unknown DNS server `{}`",
                    rule.id, rule.server_tag
                )));
            }
        }
        Ok(())
    }
}

impl OutboundsSlice {
    /// Validate enabled custom outbounds (unique generated tags, server/port).
    pub fn validate(&self) -> PanelResult<()> {
        let mut tags: HashSet<String> = HashSet::new();
        for item in self.items.iter().filter(|i| i.enabled) {
            if item.name.trim().is_empty() {
                return Err(validation(format!(
                    "custom outbound `{}` requires a name",
                    item.id
                )));
            }
            let tag = outbound_tag(&item.name);
            if !tags.insert(tag.clone()) {
                return Err(validation(format!(
                    "duplicate custom outbound tag `{tag}` (rename one of the outbounds)"
                )));
            }
            validate_outbound_address(item.protocol.server(), item.protocol.server_port(), &tag)?;
        }
        Ok(())
    }
}

/// Validate an outbound server address + port.
fn validate_outbound_address(server: &str, port: u16, tag: &str) -> PanelResult<()> {
    if server.trim().is_empty() {
        return Err(validation(format!(
            "custom outbound `{tag}` requires a server address"
        )));
    }
    if port == 0 {
        return Err(validation(format!(
            "custom outbound `{tag}` has invalid port 0"
        )));
    }
    Ok(())
}

/// Build a [`PanelError::Validation`] from a message.
fn validation(message: impl Into<String>) -> PanelError {
    PanelError::Validation(message.into())
}

#[cfg(test)]
#[path = "tests/schema_tests.rs"]
mod tests;
