//! Cache serialization types for remote Snippet aggregation.
//!
//! Rewrite rule `Regex` is persisted as pattern string, recompiled on readback.

#[cfg(feature = "mitm")]
use pp_mitm::{Phase, RewriteKind, RewriteRule, ScriptRule};
use pp_script::{ScriptKind, TaskScript};
use serde::{Deserialize, Serialize};

use crate::import::ConfigMeta;
#[cfg(feature = "mitm")]
use crate::import::ImportedConfig;

/// Cached Snippet aggregation result (JSON persistence).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CachedRemoteConfig {
    #[cfg(feature = "mitm")]
    pub rewrites: Vec<CachedRewriteRule>,
    #[cfg(feature = "mitm")]
    pub scripts: Vec<CachedScriptRule>,
    pub task_scripts: Vec<TaskScript>,
    pub hostnames: Vec<String>,
    pub meta: Option<ConfigMeta>,
}

impl CachedRemoteConfig {
    /// Build from parsed [`ImportedConfig`].
    #[cfg(feature = "mitm")]
    pub fn from_imported(imported: &ImportedConfig) -> Self {
        Self {
            rewrites: imported
                .rewrites
                .iter()
                .map(CachedRewriteRule::from)
                .collect(),
            scripts: imported
                .scripts
                .iter()
                .map(CachedScriptRule::from)
                .collect(),
            task_scripts: imported
                .task_scripts
                .iter()
                .map(|(t, _)| t.clone())
                .collect(),
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
            #[cfg(feature = "mitm")]
            rewrites: self
                .rewrites
                .into_iter()
                .filter_map(|r| r.try_into().ok())
                .collect(),
            #[cfg(feature = "mitm")]
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

#[cfg(feature = "mitm")]
impl From<&RewriteRule> for CachedRewriteRule {
    fn from(rule: &RewriteRule) -> Self {
        Self {
            pattern: rule.pattern.as_str().to_string(),
            kind: CachedRewriteKind::from(&rule.kind),
        }
    }
}

#[cfg(feature = "mitm")]
impl TryFrom<CachedRewriteRule> for RewriteRule {
    type Error = regex::Error;

    fn try_from(cached: CachedRewriteRule) -> Result<Self, Self::Error> {
        Ok(RewriteRule {
            pattern: regex::Regex::new(&cached.pattern)?,
            kind: cached.kind.try_into()?,
        })
    }
}

/// Cache serialization of rewrite rule kind.
///
/// 兼容说明：`Reject` 保留 unit 形式（映射为 404 空 body）以读取旧缓存；
/// 富语义的拒绝规则用 [`CachedRewriteKind::RejectResponse`]；`BodyRewrite` 的
/// `body_pattern` 以 `#[serde(default)]` 追加，旧缓存缺字段时回退 `None`
/// （复用 URL pattern 的旧行为）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CachedRewriteKind {
    UrlRewrite {
        target: String,
    },
    Redirect {
        status: u16,
        target: String,
    },
    HeaderRewrite {
        phase: CachedPhase,
        name: String,
        value: Option<String>,
    },
    HeaderValueRewrite {
        phase: CachedPhase,
        name: String,
        value_pattern: String,
        replacement: String,
    },
    HeaderBlockRewrite {
        phase: CachedPhase,
        block_pattern: String,
        replacement: String,
    },
    BodyRewrite {
        phase: CachedPhase,
        #[serde(default)]
        body_pattern: Option<String>,
        replacement: String,
    },
    Mock {
        status: u16,
        body: String,
        headers: Vec<(String, String)>,
    },
    Reject,
    RejectResponse {
        status: u16,
        body: Vec<u8>,
        content_type: Option<String>,
    },
}

#[cfg(feature = "mitm")]
impl From<&RewriteKind> for CachedRewriteKind {
    fn from(kind: &RewriteKind) -> Self {
        match kind {
            RewriteKind::UrlRewrite { target } => CachedRewriteKind::UrlRewrite {
                target: target.clone(),
            },
            RewriteKind::Redirect { status, target } => CachedRewriteKind::Redirect {
                status: *status,
                target: target.clone(),
            },
            RewriteKind::HeaderRewrite { phase, name, value } => CachedRewriteKind::HeaderRewrite {
                phase: CachedPhase::from(*phase),
                name: name.clone(),
                value: value.clone(),
            },
            RewriteKind::HeaderValueRewrite {
                phase,
                name,
                value_pattern,
                replacement,
            } => CachedRewriteKind::HeaderValueRewrite {
                phase: CachedPhase::from(*phase),
                name: name.clone(),
                value_pattern: value_pattern.as_str().to_string(),
                replacement: replacement.clone(),
            },
            RewriteKind::HeaderBlockRewrite {
                phase,
                block_pattern,
                replacement,
            } => CachedRewriteKind::HeaderBlockRewrite {
                phase: CachedPhase::from(*phase),
                block_pattern: block_pattern.as_str().to_string(),
                replacement: replacement.clone(),
            },
            RewriteKind::BodyRewrite {
                phase,
                body_pattern,
                replacement,
            } => CachedRewriteKind::BodyRewrite {
                phase: CachedPhase::from(*phase),
                body_pattern: body_pattern.as_ref().map(|p| p.as_str().to_string()),
                replacement: replacement.clone(),
            },
            RewriteKind::Mock {
                status,
                body,
                headers,
            } => CachedRewriteKind::Mock {
                status: *status,
                body: String::from_utf8_lossy(body).into_owned(),
                headers: headers.clone(),
            },
            RewriteKind::Reject {
                status: 404,
                body,
                content_type: None,
            } if body.is_empty() => CachedRewriteKind::Reject,
            RewriteKind::Reject {
                status,
                body,
                content_type,
            } => CachedRewriteKind::RejectResponse {
                status: *status,
                body: body.clone(),
                content_type: content_type.clone(),
            },
        }
    }
}

#[cfg(feature = "mitm")]
impl TryFrom<CachedRewriteKind> for RewriteKind {
    type Error = regex::Error;

    fn try_from(kind: CachedRewriteKind) -> Result<Self, Self::Error> {
        Ok(match kind {
            CachedRewriteKind::UrlRewrite { target } => RewriteKind::UrlRewrite { target },
            CachedRewriteKind::Redirect { status, target } => {
                RewriteKind::Redirect { status, target }
            }
            CachedRewriteKind::HeaderRewrite { phase, name, value } => RewriteKind::HeaderRewrite {
                phase: phase.into(),
                name,
                value,
            },
            CachedRewriteKind::HeaderValueRewrite {
                phase,
                name,
                value_pattern,
                replacement,
            } => RewriteKind::HeaderValueRewrite {
                phase: phase.into(),
                name,
                value_pattern: regex::Regex::new(&value_pattern)?,
                replacement,
            },
            CachedRewriteKind::HeaderBlockRewrite {
                phase,
                block_pattern,
                replacement,
            } => RewriteKind::HeaderBlockRewrite {
                phase: phase.into(),
                block_pattern: regex::Regex::new(&block_pattern)?,
                replacement,
            },
            CachedRewriteKind::BodyRewrite {
                phase,
                body_pattern,
                replacement,
            } => RewriteKind::BodyRewrite {
                phase: phase.into(),
                body_pattern: body_pattern.map(|p| regex::Regex::new(&p)).transpose()?,
                replacement,
            },
            CachedRewriteKind::Mock {
                status,
                body,
                headers,
            } => RewriteKind::Mock {
                status,
                body: body.into_bytes(),
                headers,
            },
            CachedRewriteKind::Reject => RewriteKind::reject(),
            CachedRewriteKind::RejectResponse {
                status,
                body,
                content_type,
            } => RewriteKind::Reject {
                status,
                body,
                content_type,
            },
        })
    }
}

/// Cache serialization of HTTP phase.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CachedPhase {
    Request,
    Response,
}

#[cfg(feature = "mitm")]
impl From<Phase> for CachedPhase {
    fn from(phase: Phase) -> Self {
        match phase {
            Phase::Request => CachedPhase::Request,
            Phase::Response => CachedPhase::Response,
        }
    }
}

#[cfg(feature = "mitm")]
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

#[cfg(feature = "mitm")]
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

#[cfg(feature = "mitm")]
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
