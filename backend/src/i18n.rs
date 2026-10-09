use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct LocalizedMessage {
    pub key: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

pub fn message(key: &str, params: &[(&str, &str)]) -> LocalizedMessage {
    LocalizedMessage {
        key: key.to_string(),
        params: params
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
    }
}

impl fmt::Display for LocalizedMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {:?}", self.key, self.params)
    }
}
