use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::metadata::SystemMetadata;

const DEFAULT_SSH_PORT: u16 = 22;
const DEFAULT_REMOTE_PATH: &str = "~";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerRegistry {
    #[serde(default)]
    pub servers: HashMap<String, ServerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    pub host: String,
    pub user: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_remote_path")]
    pub remote_path: String,
    pub identity: Option<String>,
    #[serde(default)]
    pub auth: AuthMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<SystemMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    #[default]
    Auto,
    Agent,
    Key,
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Agent => f.write_str("agent"),
            Self::Key => f.write_str("key"),
        }
    }
}

fn default_port() -> u16 {
    DEFAULT_SSH_PORT
}

fn default_remote_path() -> String {
    DEFAULT_REMOTE_PATH.to_string()
}

impl ServerRegistry {
    /// # Errors
    ///
    /// Returns an error if the config file exists but cannot be read or parsed.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    /// # Errors
    ///
    /// Returns an error if the config directory cannot be created or the file
    /// cannot be written.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o700);
                std::fs::set_permissions(parent, perms)?;
            }
        }
        let content = toml::to_string_pretty(self)?;

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(content.as_bytes())?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&path, content)?;
        }

        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if the platform config directory cannot be determined.
    pub fn config_path() -> Result<PathBuf> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("ssh-hub").join("servers.toml"))
    }

    /// Look up a server entry by its alias name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ServerEntry> {
        self.servers.get(name)
    }

    /// Insert or replace a server entry.
    pub fn insert(&mut self, name: String, entry: ServerEntry) {
        self.servers.insert(name, entry);
    }

    /// Remove a server entry, returning it if it existed.
    pub fn remove(&mut self, name: &str) -> Option<ServerEntry> {
        self.servers.remove(name)
    }
}
