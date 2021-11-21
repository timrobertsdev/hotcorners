use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub delay: Option<u64>,
}
