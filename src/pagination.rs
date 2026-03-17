use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::entities_v2::error::{ErrorType, PpdcError};

pub fn deserialize_one_or_many<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }

    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(value) => Ok(vec![value]),
        OneOrMany::Many(values) => Ok(values),
    }
}

pub fn parse_repeated_query_param<T>(raw_query: Option<&str>, key: &str) -> Result<Vec<T>, PpdcError>
where
    T: for<'de> Deserialize<'de>,
{
    let mut values = Vec::new();

    let Some(raw_query) = raw_query else {
        return Ok(values);
    };

    for pair in raw_query.split('&') {
        if pair.is_empty() {
            continue;
        }

        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let decoded_key = urlencoding::decode(raw_key).map_err(|error| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid query parameter key encoding: {}", error),
            )
        })?;

        if decoded_key != key {
            continue;
        }

        let decoded_value = urlencoding::decode(raw_value).map_err(|error| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                format!("Invalid query parameter value encoding for {}: {}", key, error),
            )
        })?;

        let parsed_value = serde_json::from_value::<T>(Value::String(decoded_value.into_owned()))
            .map_err(|error| {
                PpdcError::new(
                    400,
                    ErrorType::ApiError,
                    format!("Invalid query parameter value for {}: {}", key, error),
                )
            })?;

        values.push(parsed_value);
    }

    Ok(values)
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    offset: Option<i64>,
    limit: Option<i64>,
}

impl PaginationParams {
    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0)
    }

    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(20)
    }
}
