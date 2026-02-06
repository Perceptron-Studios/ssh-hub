use crate::connection::ConnectionPool;
use super::schema::DisconnectInput;

pub async fn handle(pool: &ConnectionPool, input: DisconnectInput) -> String {
    match pool.remove(&input.server).await {
        Some(_) => format!("Disconnected from '{}'", input.server),
        None => format!("Error: '{}' is not connected.", input.server),
    }
}
