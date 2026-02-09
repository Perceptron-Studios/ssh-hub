use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use russh::client::Handle;
use russh::keys::agent::client::AgentClient;
use russh::keys::{load_secret_key, Algorithm, HashAlg, PrivateKeyWithHashAlg, PublicKey};

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
            let key_path = params
                .identity
                .as_ref()
                .ok_or_else(|| anyhow!("Auth method is 'key' but no identity file specified"))?;
            if try_key_auth(session, &params.user, key_path).await? {
                Ok(())
            } else {
                Err(anyhow!("Key authentication failed"))
            }
        }
    }
}

/// Auto auth: try all methods in order.
///
/// Order: explicit identity (highest signal) → SSH agent → default key paths.
async fn authenticate_auto(
    session: &mut Handle<SshHandler>,
    params: &ConnectionParams,
) -> Result<()> {
    let mut methods_tried = Vec::new();

    // 1. Explicit identity file (user specified — highest signal)
    if let Some(key_path) = &params.identity {
        tracing::debug!("Trying identity file: {:?}", key_path);
        if try_key_auth(session, &params.user, key_path).await? {
            tracing::debug!("Authenticated via identity file");
            return Ok(());
        }
        methods_tried.push("identity file");
    }

    // 2. SSH agent
    match try_agent_auth(session, &params.user).await {
        Ok(()) => return Ok(()),
        Err(e) => {
            tracing::debug!("Agent auth failed: {}", e);
            methods_tried.push("agent");
        }
    }

    // 3. Default key paths
    for key_name in &["id_ed25519", "id_rsa", "id_ecdsa"] {
        let key_path = dirs::home_dir()
            .map(|h| h.join(".ssh").join(key_name))
            .filter(|p| p.exists());

        if let Some(key_path) = key_path {
            tracing::debug!("Trying default key: {:?}", key_path);
            if try_key_auth(session, &params.user, &key_path).await? {
                tracing::debug!("Authenticated via {:?}", key_path);
                return Ok(());
            }
        }
    }
    methods_tried.push("default keys");

    Err(anyhow!(
        "Authentication failed. Tried: {}. Check your credentials and run 'ssh-hub add' to reconfigure.",
        methods_tried.join(", ")
    ))
}

/// Query the server's preferred RSA hash algorithm once.
/// Returns None if the server doesn't advertise preferences (defaults to SHA-256).
async fn query_rsa_hash(session: &mut Handle<SshHandler>) -> Option<HashAlg> {
    match session.best_supported_rsa_hash().await {
        Ok(Some(hash_alg)) => {
            tracing::debug!("Server prefers RSA hash: {:?}", hash_alg);
            hash_alg
        }
        Ok(None) => {
            tracing::debug!("Server didn't advertise RSA hash preference, defaulting to SHA-256");
            Some(HashAlg::Sha256)
        }
        Err(e) => {
            tracing::debug!("Failed to query server RSA hash support: {}", e);
            Some(HashAlg::Sha256)
        }
    }
}

/// Get the RSA hash for a key, using the cached value for RSA keys.
fn rsa_hash_for_key(key: &PublicKey, cached_rsa_hash: Option<HashAlg>) -> Option<HashAlg> {
    if matches!(key.algorithm(), Algorithm::Rsa { .. }) {
        cached_rsa_hash
    } else {
        None
    }
}

/// Maximum agent keys to try before giving up (avoids "too many auth failures").
const MAX_AGENT_KEYS: usize = 10;

/// Try SSH agent authentication.
async fn try_agent_auth(session: &mut Handle<SshHandler>, user: &str) -> Result<()> {
    let mut agent = AgentClient::connect_env()
        .await
        .context("Failed to connect to SSH agent (is SSH_AUTH_SOCK set?)")?;

    let identities = agent
        .request_identities()
        .await
        .context("Failed to list keys from SSH agent")?;

    if identities.is_empty() {
        return Err(anyhow!("SSH agent has no keys. Run 'ssh-add' first."));
    }

    let total = identities.len();
    let try_count = total.min(MAX_AGENT_KEYS);
    tracing::debug!("SSH agent has {} key(s), trying {}", total, try_count);

    // Query RSA hash once and cache for all keys
    let cached_rsa_hash = query_rsa_hash(session).await;

    for (i, key) in identities.iter().take(try_count).enumerate() {
        let hash_alg = rsa_hash_for_key(key, cached_rsa_hash);
        tracing::debug!(
            "Trying agent key {}/{}: {:?} (hash: {:?})",
            i + 1,
            try_count,
            key.algorithm(),
            hash_alg,
        );
        match session
            .authenticate_publickey_with(user, key.clone(), hash_alg, &mut agent)
            .await
        {
            Ok(result) if result.success() => {
                tracing::debug!("Authenticated via SSH agent (key {}/{})", i + 1, try_count);
                return Ok(());
            }
            Ok(result) => {
                tracing::debug!(
                    "Agent key {}/{} rejected by server: {:?}",
                    i + 1,
                    try_count,
                    result
                );
            }
            Err(e) => {
                tracing::debug!("Agent key {}/{} error: {}", i + 1, try_count, e);
            }
        }
    }

    Err(anyhow!(
        "SSH agent has {total} key(s) but none of the first {try_count} were accepted",
    ))
}

/// Try to authenticate with a specific key file.
async fn try_key_auth(
    session: &mut Handle<SshHandler>,
    user: &str,
    key_path: &Path,
) -> Result<bool> {
    let key_path_owned = key_path.to_path_buf();
    let key = match tokio::task::spawn_blocking(move || load_secret_key(&key_path_owned, None))
        .await
        .context("Key loading task panicked")?
    {
        Ok(k) => k,
        Err(e) => {
            tracing::debug!("Failed to load key {:?}: {}", key_path, e);
            return Ok(false);
        }
    };

    let cached_rsa_hash = query_rsa_hash(session).await;
    let hash_alg = rsa_hash_for_key(key.public_key(), cached_rsa_hash);
    let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);

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
