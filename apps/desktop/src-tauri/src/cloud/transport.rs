use super::ConnectionState;
use reqwest::Client;
use std::{sync::LazyLock, time::Duration};

pub(super) static CATALOG_CLIENT: LazyLock<Result<Client, ConnectionState>> =
    LazyLock::new(|| build(8));
pub(super) static JSON_CLIENT: LazyLock<Result<Client, ConnectionState>> =
    LazyLock::new(|| build(10));

fn build(connect_seconds: u64) -> Result<Client, ConnectionState> {
    crate::http_transport::model_client(Duration::from_secs(connect_seconds))
        .pool_max_idle_per_host(1)
        .build()
        .map_err(|_| ConnectionState::ServiceError)
}
