use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};
use uuid::Uuid;

use super::{CachedSubscriptionContent, Subscription};

/// Subscription storage: read/write `data_dir/subscriptions.json` (load / save /
/// add / remove / set_enabled / set_profile_id).
#[derive(Debug, Clone)]
pub struct SubscriptionStore {
    data_dir: PathBuf,
}

impl SubscriptionStore {
    /// Create storage based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/subscriptions.json`.
    pub fn file(&self) -> PathBuf {
        self.data_dir.join("subscriptions.json")
    }

    /// Read subscription list; returns empty list when file is missing, logs
    /// warning and falls back to empty list when corrupted.
    pub fn load(&self) -> PanelResult<Vec<Subscription>> {
        let path = self.file();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&path)?;
        match serde_json::from_str(&text) {
            Ok(subs) => Ok(subs),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "subscriptions.json unreadable, fall back to empty list"
                );
                Ok(Vec::new())
            }
        }
    }

    /// Save subscription list to `data_dir/subscriptions.json`.
    pub fn save(&self, subs: &[Subscription]) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(subs)?;
        std::fs::write(self.file(), text)?;
        Ok(())
    }

    /// Append a subscription and persist.
    pub fn add(
        &self,
        name: &str,
        url: &str,
        enabled: bool,
        user_agent: Option<&str>,
    ) -> PanelResult<Subscription> {
        let mut subs = self.load()?;
        let sub = Subscription {
            id: Uuid::new_v4(),
            name: name.to_string(),
            url: url.to_string(),
            enabled,
            userinfo: None,
            node_count: 0,
            error: None,
            user_agent: user_agent.map(str::to_string),
            format: None,
            profile_id: None,
        };
        subs.push(sub.clone());
        self.save(&subs)?;
        Ok(sub)
    }

    /// Remove subscription by id and persist; silently returns when not exists.
    pub fn remove(&self, id: Uuid) -> PanelResult<()> {
        let mut subs = self.load()?;
        let before = subs.len();
        subs.retain(|s| s.id != id);
        if subs.len() == before {
            return Ok(());
        }
        self.save(&subs)
    }

    /// Toggle subscription enabled state by id and persist; silently returns
    /// when not exists.
    pub fn set_enabled(&self, id: Uuid, enabled: bool) -> PanelResult<()> {
        let mut subs = self.load()?;
        let mut found = false;
        for sub in &mut subs {
            if sub.id == id {
                sub.enabled = enabled;
                found = true;
            }
        }
        if !found {
            return Ok(());
        }
        self.save(&subs)
    }

    /// Set subscription associated override template (`None` = cancel
    /// association) and persist; errors when subscription does not exist.
    pub fn set_profile_id(&self, id: Uuid, profile_id: Option<Uuid>) -> PanelResult<()> {
        let mut subs = self.load()?;
        let target = subs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| PanelError::Client(format!("Subscription does not exist (id: {id})")))?;
        target.profile_id = profile_id;
        self.save(&subs)
    }

    /// Update subscription name / url / user_agent by id and persist; errors
    /// when subscription does not exist.
    ///
    /// When URL changes, clear last fetch cache (`userinfo` / `node_count`) to
    /// avoid old data misleading display under new URL; cache is retained when
    /// URL unchanged.
    pub fn update(
        &self,
        id: Uuid,
        name: &str,
        url: &str,
        user_agent: Option<&str>,
    ) -> PanelResult<()> {
        let mut subs = self.load()?;
        let target = subs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| PanelError::Client(format!("Subscription does not exist (id: {id})")))?;
        let url_changed = target.url != url;
        target.name = name.to_string();
        target.url = url.to_string();
        target.user_agent = user_agent.map(str::to_string);
        if url_changed {
            target.userinfo = None;
            target.node_count = 0;
            target.format = None;
            // URL change means old URL fetched content cache is also invalid,
            // delete together.
            self.clear_cached_content(id);
        }
        self.save(&subs)
    }

    /// `data_dir/subscription_cache/<id>.json`.
    pub fn cache_file(&self, id: Uuid) -> PathBuf {
        self.data_dir
            .join("subscription_cache")
            .join(format!("{id}.json"))
    }

    /// Write subscription content cache to
    /// `data_dir/subscription_cache/<id>.json`.
    pub fn write_cached_content(
        &self,
        id: Uuid,
        content: &CachedSubscriptionContent,
    ) -> PanelResult<()> {
        let dir = self.data_dir.join("subscription_cache");
        std::fs::create_dir_all(&dir)?;
        let text = serde_json::to_string_pretty(content)?;
        std::fs::write(dir.join(format!("{id}.json")), text)?;
        Ok(())
    }

    /// Read subscription content cache; returns `None` when file is missing /
    /// unreadable / corrupted (logs debug / warn, does not error to caller,
    /// caller falls back to remote fetch).
    pub fn load_cached_content(&self, id: Uuid) -> Option<CachedSubscriptionContent> {
        let path = self.cache_file(id);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(path = %path.display(), "subscription cache missing");
                return None;
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "subscription cache unreadable, ignore"
                );
                return None;
            }
        };
        match serde_json::from_str(&text) {
            Ok(content) => Some(content),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "subscription cache corrupted, ignore"
                );
                None
            }
        }
    }

    /// Delete subscription content cache file (URL change etc.); silently
    /// returns when file does not exist.
    pub fn clear_cached_content(&self, id: Uuid) {
        let path = self.cache_file(id);
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::debug!(path = %path.display(), "subscription cache cleared"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to clear subscription cache"
            ),
        }
    }
}
