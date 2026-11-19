use reqwest::{Client, ClientBuilder, redirect::Policy};
use std::time::Duration;

pub(crate) fn model_client(connect_timeout: Duration) -> ClientBuilder {
    Client::builder()
        .redirect(Policy::none())
        .connect_timeout(connect_timeout)
}
