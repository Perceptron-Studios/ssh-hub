use anyhow::{anyhow, Context, Result};
use russh::client::Handle;
use russh::keys::agent::client::AgentClient;
use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use std::sync::Arc;

use super::session::{ConnectionParams, SshHandler};

/// Authenticate with the SSH server using the configured auth method.
pub async fn authenticate(
    session: &mut Handle<SshHandler>,
    params: &ConnectionParams,
) -> Result<()> {
    match &params.auth_method {
        crate::server_registry::AuthMethod::Auto => authenticate_auto(session, params).await,
        crate::server_registry::AuthMethod::Agent => try_agent_auth(session, &params.user).await,
        crate::server_registry::AuthMethod::Key => {
            let key_path = params.identity.as_ref().ok_or_else(|| {
                anyhow!("Auth method is 'key' but no identity file specified")
            })?;
            if try_key_auth(session, &params.user, key_path).await? {
                Ok(())
            } else {
                Err(anyhow!("Key authentication failed"))
            }
        }
    }
}

/// Auto auth: try all methods in order.
async fn authenticate_auto(
    session: &mut Handle<SshHandler>,
    params: &ConnectionParams,
) -> Result<()> {
    let mut methods_tried = Vec::new();

    // 1. SSH agent
    match try_agent_auth(session, &params.user).await {
        Ok(()) => return Ok(()),
        Err(e) => {
            tracing::debug!("Agent auth failed: {}", e);
            methods_tried.push("agent");
        }
    }

    // 2. Explicit identity file
    if let Some(key_path) = &params.identity {
        tracing::debug!("Trying identity file: {:?}", key_path);
        if try_key_auth(session, &params.user, key_path).await? {
            tracing::info!("Authenticated via identity file");
            return Ok(());
        }
        methods_tried.push("identity file");
    }

    // 3. Default key paths
    for key_name in &["id_ed25519", "id_rsa", "id_ecdsa"] {
        let key_path = dirs::home_dir()
            .map(|h| h.join(".ssh").join(key_name))
            .filter(|p| p.exists());

        if let Some(key_path) = key_path {
            tracing::debug!("Trying default key: {:?}", key_path);
            if try_key_auth(session, &params.user, &key_path).await? {
                tracing::info!("Authenticated via {:?}", key_path);
                return Ok(());
            }
        }
    }
    methods_tried.push("default keys");

    Err(anyhow!(
        "Authentication failed. Tried: {}. Run 'ssh-hub setup <name>' to configure credentials.",
        methods_tried.join(", ")
    ))
}

/// Try SSH agent authentication.
async fn try_agent_auth(
    session: &mut Handle<SshHandler>,
    user: &str,
) -> Result<()> {
    let mut agent = AgentClient::connect_env().await
        .context("Failed to connect to SSH agent (is SSH_AUTH_SOCK set?)")?;

    let identities = agent.request_identities().await
        .context("Failed to list keys from SSH agent")?;

    if identities.is_empty() {
        return Err(anyhow!("SSH agent has no keys. Run 'ssh-add' first."));
    }

    for key in &identities {
        match session.authenticate_publickey_with(user, key.clone(), None, &mut agent).await {
            Ok(result) if result.success() => {
                tracing::info!("Authenticated via SSH agent");
                return Ok(());
            }
            _ => continue,
        }
    }

    Err(anyhow!("SSH agent has {} key(s) but none were accepted", identities.len()))
}

/// Try to authenticate with a specific key file.
async fn try_key_auth(
    session: &mut Handle<SshHandler>,
    user: &str,
    key_path: &std::path::PathBuf,
) -> Result<bool> {
    let key = match load_secret_key(key_path, None) {
        Ok(k) => k,
        Err(e) => {
            tracing::debug!("Failed to load key {:?}: {}", key_path, e);
            return Ok(false);
        }
    };

    let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), None);

    match session.authenticate_publickey(user, key_with_alg).await {
        Ok(result) => {
            if result.success() {
                return Ok(true);
            }
            tracing::debug!("Key auth failed for {:?}", key_path);
            Ok(false)
        }
        Err(e) => {
            tracing::debug!("Key auth error for {:?}: {}", key_path, e);
            Ok(false)
        }
    }
}
