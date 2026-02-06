use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DisconnectInput {
    #[schemars(description = "Name of the server to disconnect")]
    pub server: String,
}
