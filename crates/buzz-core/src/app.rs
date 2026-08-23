//! App protocol types, kind 9038 command schema, and callback limits.
//!
//! This module is the wire contract only. Relays execute commands and persist
//! Apps elsewhere; clients must not treat these types as authorization.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Cryptographically random App callback secret length in bytes.
pub const APP_SECRET_BYTES: usize = 32;
/// Maximum accepted HTTP callback body size in bytes (64 KiB).
pub const APP_CALLBACK_BODY_MAX_BYTES: usize = 65_536;
/// Maximum serialized callback `metadata` object size in bytes (32 KiB).
pub const APP_CALLBACK_METADATA_MAX_BYTES: usize = 32_768;
/// Maximum JSON nesting depth accepted in callback `metadata`.
pub const APP_CALLBACK_METADATA_MAX_DEPTH: usize = 16;
/// Maximum recursive object-member plus array-element count in callback `metadata`.
pub const APP_CALLBACK_METADATA_MAX_NODES: usize = 1_024;

const APP_NAME_MAX_CHARS: usize = 128;
const APP_DESCRIPTION_MAX_CHARS: usize = 2_048;
const APP_ICON_URL_MAX_BYTES: usize = 4_096;

/// Failure to parse or validate a kind 9038 App admin command.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AppCommandError {
    /// Command JSON or field values are not valid.
    #[error("{0}")]
    Invalid(String),
}

/// Public lifecycle status of a community App.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    /// The App may receive callbacks.
    Active,
    /// The App rejects callbacks; metadata remains queryable.
    Disabled,
}

/// Kind 9038 admin command body.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppAdminCommand {
    /// Create a new App.
    Create {
        /// Display name.
        name: String,
        /// Optional public description.
        description: Option<String>,
        /// Optional icon URL.
        icon_url: Option<String>,
    },
    /// Update public metadata of an existing App.
    Update {
        /// Canonical App UUID.
        #[serde(deserialize_with = "deserialize_canonical_app_id")]
        app_id: Uuid,
        /// Replacement display name.
        name: Option<String>,
        /// Replacement or cleared description.
        description: Option<String>,
        /// Replacement or cleared icon URL.
        icon_url: Option<String>,
    },
    /// Replace the App callback secret.
    RotateSecret {
        /// Canonical App UUID.
        #[serde(deserialize_with = "deserialize_canonical_app_id")]
        app_id: Uuid,
    },
    /// Enable callbacks for a disabled App.
    Enable {
        /// Canonical App UUID.
        #[serde(deserialize_with = "deserialize_canonical_app_id")]
        app_id: Uuid,
    },
    /// Disable callbacks while keeping metadata queryable.
    Disable {
        /// Canonical App UUID.
        #[serde(deserialize_with = "deserialize_canonical_app_id")]
        app_id: Uuid,
    },
}

/// Parse and validate kind 9038 command JSON.
pub fn parse_app_admin_command(content: &str) -> Result<AppAdminCommand, AppCommandError> {
    let command: AppAdminCommand =
        serde_json::from_str(content).map_err(|err| AppCommandError::Invalid(err.to_string()))?;
    command.validate()
}

/// Validate an App icon URL against the protocol contract.
///
/// Empty string is accepted (clear). Non-empty values must be at most 4096
/// UTF-8 bytes and use a safe `http://`, `https://`, or `data:image/` form
/// with no whitespace or control characters.
pub fn validate_app_icon_url(icon_url: &str) -> Result<(), AppCommandError> {
    if icon_url.is_empty() {
        return Ok(());
    }
    if icon_url.len() > APP_ICON_URL_MAX_BYTES {
        return Err(AppCommandError::Invalid(format!(
            "icon_url exceeds {APP_ICON_URL_MAX_BYTES} UTF-8 bytes"
        )));
    }
    if icon_url
        .chars()
        .any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(AppCommandError::Invalid(
            "icon_url contains invalid characters".into(),
        ));
    }
    if icon_url.starts_with("data:image/")
        || icon_url.starts_with("https://")
        || icon_url.starts_with("http://")
    {
        return Ok(());
    }
    Err(AppCommandError::Invalid(
        "icon_url must be an http(s) URL or data:image/ URL".into(),
    ))
}

/// Parse a canonical lowercase hyphenated App UUID.
pub fn parse_canonical_app_id(s: &str) -> Result<Uuid, AppCommandError> {
    if !is_canonical_hyphenated_uuid(s) {
        return Err(invalid_app_id());
    }
    Uuid::parse_str(s).map_err(|_| invalid_app_id())
}

impl AppAdminCommand {
    fn validate(self) -> Result<Self, AppCommandError> {
        match self {
            Self::Create {
                name,
                description,
                icon_url,
            } => Ok(Self::Create {
                name: validate_name(&name)?,
                description: normalize_create_description(description)?,
                icon_url: normalize_create_icon(icon_url)?,
            }),
            Self::Update {
                app_id,
                name,
                description,
                icon_url,
            } => {
                if name.is_none() && description.is_none() && icon_url.is_none() {
                    return Err(AppCommandError::Invalid(
                        "update must contain at least one of name, description, or icon_url".into(),
                    ));
                }
                Ok(Self::Update {
                    app_id,
                    name: match name {
                        Some(value) => Some(validate_name(&value)?),
                        None => None,
                    },
                    description: match description {
                        Some(value) => {
                            validate_description(&value)?;
                            Some(value)
                        }
                        None => None,
                    },
                    icon_url: match icon_url {
                        Some(value) => {
                            validate_app_icon_url(&value)?;
                            Some(value)
                        }
                        None => None,
                    },
                })
            }
            other => Ok(other),
        }
    }
}

fn validate_name(name: &str) -> Result<String, AppCommandError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppCommandError::Invalid("name must be non-empty".into()));
    }
    if trimmed.chars().count() > APP_NAME_MAX_CHARS {
        return Err(AppCommandError::Invalid(format!(
            "name exceeds {APP_NAME_MAX_CHARS} Unicode scalar values"
        )));
    }
    Ok(trimmed.to_string())
}

fn validate_description(description: &str) -> Result<&str, AppCommandError> {
    if description.chars().count() > APP_DESCRIPTION_MAX_CHARS {
        return Err(AppCommandError::Invalid(format!(
            "description exceeds {APP_DESCRIPTION_MAX_CHARS} Unicode scalar values"
        )));
    }
    Ok(description)
}

fn normalize_create_description(
    description: Option<String>,
) -> Result<Option<String>, AppCommandError> {
    match description {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => {
            validate_description(&value)?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

fn normalize_create_icon(icon_url: Option<String>) -> Result<Option<String>, AppCommandError> {
    match icon_url {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => {
            validate_app_icon_url(&value)?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

fn deserialize_canonical_app_id<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    parse_canonical_app_id(&raw).map_err(serde::de::Error::custom)
}

fn is_canonical_hyphenated_uuid(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    s.bytes().enumerate().all(|(i, b)| match i {
        8 | 13 | 18 | 23 => b == b'-',
        _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
    })
}

fn invalid_app_id() -> AppCommandError {
    AppCommandError::Invalid("app_id must be a canonical lowercase hyphenated UUID".into())
}
