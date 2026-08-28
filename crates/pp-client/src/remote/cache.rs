//! Cache serialization types for remote Snippet aggregation.
//!
//! Rewrite rule `Regex` is persisted as pattern string, recompiled on readback.

use pp_mitm::{Phase, RewriteKind, RewriteRule, ScriptRule};
use pp_script::{ScriptKind, TaskScript};
use serde::{Deserialize, Serialize};

use crate::import::{ConfigMeta, ImportedConfig};

/// Cached Snippet aggregation result (JSON persistence).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedRemoteConfig {
    pub rewrites: Vec<CachedRewriteRule>,
    pub scripts: Vec<CachedScriptRule>,
    pub task_scripts: Vec<TaskScript>,
    pub hostnames: Vec<String>,
    pub meta: Option<ConfigMeta>,
}

impl CachedRemoteConfig {
    /// Build from parsed [`ImportedConfig`].
    pub fn from_imported(imported: &ImportedConfig) -> Self {
        Self {
            rewrites: imported.rewrites.iter().map(CachedRewriteRule::from).collect(),
            scripts: imported.scripts.iter().map(CachedScriptRule::from).collect(),
            task_scripts: imported.task_scripts.iter().map(|(t, _)| t.clone()).collect(),
            hostnames: imported.hostnames.clone(),
            meta: if imported.meta == ConfigMeta::default() {
                None
            } else {
                Some(imported.meta.clone())
            },
        }
    }

    /// Convert cache to runtime merged config (recompile regex patterns).
    pub fn into_merged(self) -> super::MergedRemoteConfig {
        super::MergedRemoteConfig {
            rewrites: self
                .rewrites
                .into_iter()
                .filter_map(|r| r.try_into().ok())
                .collect(),
            scripts: self
                .scripts
                .into_iter()
                .filter_map(|s| s.try_into().ok())
                .collect(),
            task_scripts: self.task_scripts,
            hostnames: self.hostnames,
            metas: self.meta.into_iter().collect(),
        }
    }
}

/// Cache serialization of a rewrite rule (regex stored as string, recompiled on readback).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRewriteRule {
    pub pattern: String,
    pub kind: CachedRewriteKind,
}

impl From<&RewriteRule> for CachedRewriteRule {
    fn from(rule: &RewriteRule) -> Self {
        Self {
            pattern: rule.pattern.as_str().to_string(),
            kind: CachedRewriteKind::from(&rule.kind),
        }
    }
}

impl TryFrom<CachedRewriteRule> for RewriteRule {
    type Error = regex::Error;

    fn try_from(cached: CachedRewriteRule) -> Result<Self, Self::Error> {
        Ok(RewriteRule {
            pattern: regex::Regex::new(&cached.pattern)?,
            kind: cached.kind.into(),
        })
    }
}

/// Cache serialization of rewrite rule kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CachedRewriteKind {
    UrlRewrite { target: String },
    HeaderRewrite {
        phase: CachedPhase,
        name: String,
        value: Option<String>,
    },
    BodyRewrite {
        phase: CachedPhase,
        replacement: String,
    },
    Mock {
        status: u16,
        body: String,
        headers: Vec<(String, String)>,
    },
    Reject,
}

impl From<&RewriteKind> for CachedRewriteKind {
    fn from(kind: &RewriteKind) -> Self {
        match kind {
            RewriteKind::UrlRewrite { target } => CachedRewriteKind::UrlRewrite {
                target: target.clone(),
            },
            RewriteKind::HeaderRewrite { phase, name, value } => CachedRewriteKind::HeaderRewrite {
                phase: CachedPhase::from(*phase),
                name: name.clone(),
                value: value.clone(),
            },
            RewriteKind::BodyRewrite { phase, replacement } => CachedRewriteKind::BodyRewrite {
                phase: CachedPhase::from(*phase),
                replacement: replacement.clone(),
            },
            RewriteKind::Mock {
                status,
                body,
                headers,
            } => CachedRewriteKind::Mock {
                status: *status,
                body: body.clone(),
                headers: headers.clone(),
            },
            RewriteKind::Reject => CachedRewriteKind::Reject,
        }
    }
}

impl From<CachedRewriteKind> for RewriteKind {
    fn from(kind: CachedRewriteKind) -> Self {
        match kind {
            CachedRewriteKind::UrlRewrite { target } => RewriteKind::UrlRewrite { target },
            CachedRewriteKind::HeaderRewrite { phase, name, value } => RewriteKind::HeaderRewrite {
                phase: phase.into(),
                name,
                value,
            },
            CachedRewriteKind::BodyRewrite { phase, replacement } => RewriteKind::BodyRewrite {
                phase: phase.into(),
                replacement,
            },
            CachedRewriteKind::Mock {
                status,
                body,
                headers,
            } => RewriteKind::Mock {
                status,
                body,
                headers,
            },
            CachedRewriteKind::Reject => RewriteKind::Reject,
        }
    }
}

/// Cache serialization of HTTP phase.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CachedPhase {
    Request,
    Response,
}

impl From<Phase> for CachedPhase {
    fn from(phase: Phase) -> Self {
        match phase {
            Phase::Request => CachedPhase::Request,
            Phase::Response => CachedPhase::Response,
        }
    }
}

impl From<CachedPhase> for Phase {
    fn from(phase: CachedPhase) -> Self {
        match phase {
            CachedPhase::Request => Phase::Request,
            CachedPhase::Response => Phase::Response,
        }
    }
}

/// Cache serialization of script hook rule (regex stored as string, recompiled on readback).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedScriptRule {
    pub name: String,
    pub kind: CachedScriptKind,
    pub pattern: String,
    pub requires_body: bool,
    pub max_size: usize,
    pub source: String,
    pub argument: Option<String>,
}

impl From<&ScriptRule> for CachedScriptRule {
    fn from(rule: &ScriptRule) -> Self {
        Self {
            name: rule.name.clone(),
            kind: CachedScriptKind::from(rule.kind),
            pattern: rule.pattern.as_str().to_string(),
            requires_body: rule.requires_body,
            max_size: rule.max_size,
            source: rule.source.clone(),
            argument: rule.argument.clone(),
        }
    }
}

impl TryFrom<CachedScriptRule> for ScriptRule {
    type Error = regex::Error;

    fn try_from(cached: CachedScriptRule) -> Result<Self, Self::Error> {
        Ok(ScriptRule {
            name: cached.name,
            kind: cached.kind.into(),
            pattern: regex::Regex::new(&cached.pattern)?,
            requires_body: cached.requires_body,
            max_size: cached.max_size,
            source: cached.source,
            argument: cached.argument,
        })
    }
}

/// Cache serialization of script kind.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CachedScriptKind {
    HttpRequest,
    HttpResponse,
}

impl From<ScriptKind> for CachedScriptKind {
    fn from(kind: ScriptKind) -> Self {
        match kind {
            ScriptKind::HttpRequest => CachedScriptKind::HttpRequest,
            ScriptKind::HttpResponse => CachedScriptKind::HttpResponse,
            ScriptKind::Cron | ScriptKind::Generic => CachedScriptKind::HttpResponse,
        }
    }
}

impl From<CachedScriptKind> for ScriptKind {
    fn from(kind: CachedScriptKind) -> Self {
        match kind {
            CachedScriptKind::HttpRequest => ScriptKind::HttpRequest,
            CachedScriptKind::HttpResponse => ScriptKind::HttpResponse,
        }
    }
}
