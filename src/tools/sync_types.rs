use serde::Serialize;

/// A single file that failed during a sync operation.
#[derive(Debug, Serialize)]
pub struct FailedTransfer {
    pub path: String,
    pub error: String,
}

/// Unified output for sync push/pull operations.
///
/// The `transferred` field lists successfully synced files,
/// and `failed` lists any files that encountered errors.
#[derive(Debug, Serialize)]
pub struct SyncOutput {
    pub transferred: Vec<String>,
    pub failed: Vec<FailedTransfer>,
}

impl SyncOutput {
    /// Shorthand for a successful result with no failures.
    pub fn success(transferred: Vec<String>) -> Self {
        Self {
            transferred,
            failed: vec![],
        }
    }

    /// Shorthand for a single-file failure with no successes.
    pub fn failure(path: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            transferred: vec![],
            failed: vec![FailedTransfer {
                path: path.into(),
                error: error.into(),
            }],
        }
    }

    /// Serialize to compact JSON.
    ///
    /// Falls back to a minimal error JSON if serialization fails, which
    /// should never happen since both fields are trivially serializable.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .unwrap_or_else(|e| format!(r#"{{"error": "serialization failed: {}"}}"#, e))
    }
}
