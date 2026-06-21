use pp_common::{CoreType, PanelResult, ProtocolType};
use serde_json::Value;

/// Abstract configuration builder trait.
/// Each supported core implements this to translate generic ProtocolConfig
/// into core-specific JSON configuration.
pub trait ConfigBuilder: Send + Sync {
    /// The core type this builder targets.
    fn core_type(&self) -> CoreType;

    /// Build inbound configuration for a given protocol.
    fn build_inbound(
        &self,
        protocol: ProtocolType,
        settings: &Value,
        tls: Option<&Value>,
    ) -> PanelResult<Value>;

    /// Build a complete core configuration combining multiple inbounds.
    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value>;
}

/// Normalized inbound config passed to builders.
#[derive(Debug, Clone)]
pub struct InboundConfig {
    pub tag: String,
    pub protocol: ProtocolType,
    pub listen: String,
    pub port: u16,
    pub settings: Value,
    pub tls: Option<Value>,
    pub sniffing: Option<Value>,
}

/// Registry of available builders.
pub struct BuilderRegistry {
    builders: Vec<Box<dyn ConfigBuilder>>,
}

impl BuilderRegistry {
    pub fn new() -> Self {
        Self {
            builders: Vec::new(),
        }
    }

    pub fn register<B: ConfigBuilder + 'static>(&mut self, builder: B) {
        self.builders.push(Box::new(builder));
    }

    pub fn get(&self, core: CoreType) -> Option<&dyn ConfigBuilder> {
        self.builders
            .iter()
            .find(|b| b.core_type() == core)
            .map(|b| b.as_ref())
    }
}

impl Default for BuilderRegistry {
    fn default() -> Self {
        let mut reg = Self::new();
        reg.register(crate::xray::XrayConfigBuilder);
        reg.register(crate::singbox::SingBoxConfigBuilder);
        reg
    }
}
