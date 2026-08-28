//! Profile storage: legacy single-file [`ProfileStore`] and multi-template [`ProfileStoreV2`].

use std::path::PathBuf;

use pp_common::{CoreType, PanelError, PanelResult};
use uuid::Uuid;

use super::{Profile, ProfileOverrides};

/// Profile storage (legacy single-file): reads/writes [`ProfileOverrides`] in
/// `data_dir/profile.json`.
///
/// Legacy single-file storage (only for compatibility with legacy callers; new code should use
/// multi-template [`ProfileStoreV2`]). Its content is migrated to `profiles.json` once during
/// the first call to [`ProfileStoreV2::load`], then deleted.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    data_dir: PathBuf,
}

impl ProfileStore {
    /// Create storage based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/profile.json`.
    pub fn profile_file(&self) -> PathBuf {
        self.data_dir.join("profile.json")
    }

    /// Read override config; returns default (empty) when file is missing, logs warning and
    /// falls back to default when corrupted.
    pub fn load(&self) -> PanelResult<ProfileOverrides> {
        let path = self.profile_file();
        if !path.exists() {
            return Ok(ProfileOverrides::default());
        }
        let text = std::fs::read_to_string(&path)?;
        match serde_json::from_str(&text) {
            Ok(overrides) => Ok(overrides),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "profile.json unreadable, fall back to defaults"
                );
                Ok(ProfileOverrides::default())
            }
        }
    }

    /// Save override config to `data_dir/profile.json`.
    pub fn save(&self, overrides: &ProfileOverrides) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(overrides)?;
        std::fs::write(self.profile_file(), text)?;
        Ok(())
    }
}

/// Multi-template Profile storage: reads/writes a list of [`Profile`] in
/// `data_dir/profiles.json`.
///
/// Pure association model: templates do not hold an enabled state, the runtime override =
/// the template associated with the currently selected subscription (see `crate::state`
/// startup flow and subscription's `profile_id`).
///
/// Legacy single-file `data_dir/profile.json` ([`ProfileStore`]) is migrated to
/// `Profile{name:"Default", core_type: SingBox}` (override content preserved as-is) during
/// the first call to [`ProfileStoreV2::load`], then deleted; no migration is performed when
/// `profiles.json` already exists.
#[derive(Debug, Clone)]
pub struct ProfileStoreV2 {
    data_dir: PathBuf,
}

impl ProfileStoreV2 {
    /// Create storage based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/profiles.json`.
    pub fn profiles_file(&self) -> PathBuf {
        self.data_dir.join("profiles.json")
    }

    /// Legacy single-file path `data_dir/profile.json` (migration source).
    pub fn legacy_file(&self) -> PathBuf {
        self.data_dir.join("profile.json")
    }

    /// Read all templates.
    ///
    /// When `profiles.json` is missing: if old `profile.json` exists, migrate it to the
    /// default template once and delete the old file; otherwise return empty list. When
    /// `profiles.json` is corrupted, log warning and fall back to empty list.
    pub fn load(&self) -> PanelResult<Vec<Profile>> {
        let path = self.profiles_file();
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            return match serde_json::from_str(&text) {
                Ok(profiles) => Ok(profiles),
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "profiles.json unreadable, fall back to empty"
                    );
                    Ok(Vec::new())
                }
            };
        }
        let legacy = self.legacy_file();
        if legacy.exists() {
            let overrides = match std::fs::read_to_string(&legacy)
                .map(|t| serde_json::from_str::<ProfileOverrides>(&t))
            {
                Ok(Ok(ov)) => ov,
                Ok(Err(e)) => {
                    tracing::warn!(
                        path = %legacy.display(),
                        error = %e,
                        "legacy profile.json unreadable, migrate with empty overrides"
                    );
                    ProfileOverrides::default()
                }
                Err(e) => {
                    tracing::warn!(
                        path = %legacy.display(),
                        error = %e,
                        "legacy profile.json unreadable, migrate with empty overrides"
                    );
                    ProfileOverrides::default()
                }
            };
            let profiles = vec![Profile {
                id: Uuid::new_v4(),
                name: "Default".to_string(),
                core_type: CoreType::SingBox,
                yaml_override: overrides.yaml_override,
                js_override: overrides.js_override,
                yaml_url: None,
                js_url: None,
            }];
            self.save(&profiles)?;
            if let Err(e) = std::fs::remove_file(&legacy) {
                tracing::warn!(
                    path = %legacy.display(),
                    error = %e,
                    "failed to remove legacy profile.json after migration"
                );
            }
            return Ok(profiles);
        }
        Ok(Vec::new())
    }

    /// Save all templates to `data_dir/profiles.json`.
    pub fn save(&self, profiles: &[Profile]) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(profiles)?;
        std::fs::write(self.profiles_file(), text)?;
        Ok(())
    }

    /// Add a new template: errors when name duplicates an existing template.
    pub fn add(&self, name: &str, core_type: CoreType) -> PanelResult<Profile> {
        let mut profiles = self.load()?;
        if profiles.iter().any(|p| p.name == name) {
            return Err(PanelError::Client(format!(
                "profile with name '{name}' already exists"
            )));
        }
        let profile = Profile {
            id: Uuid::new_v4(),
            name: name.to_string(),
            core_type,
            yaml_override: String::new(),
            js_override: String::new(),
            yaml_url: None,
            js_url: None,
        };
        profiles.push(profile.clone());
        self.save(&profiles)?;
        Ok(profile)
    }

    /// Update editable fields (name / yaml_override / js_override / yaml_url / js_url) by id;
    /// `core_type` keeps its stored value. Errors when template does not exist.
    pub fn update(&self, profile: &Profile) -> PanelResult<()> {
        let mut profiles = self.load()?;
        let target = profiles
            .iter_mut()
            .find(|p| p.id == profile.id)
            .ok_or_else(|| PanelError::Client(format!("profile {} not found", profile.id)))?;
        target.name = profile.name.clone();
        target.yaml_override = profile.yaml_override.clone();
        target.js_override = profile.js_override.clone();
        target.yaml_url = profile.yaml_url.clone();
        target.js_url = profile.js_url.clone();
        self.save(&profiles)
    }

    /// Remove template by id; errors when it does not exist.
    pub fn remove(&self, id: Uuid) -> PanelResult<()> {
        let mut profiles = self.load()?;
        let before = profiles.len();
        profiles.retain(|p| p.id != id);
        if profiles.len() == before {
            return Err(PanelError::Client(format!("profile {id} not found")));
        }
        self.save(&profiles)
    }
}
