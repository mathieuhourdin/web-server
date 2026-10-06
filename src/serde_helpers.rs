use serde::{Deserialize, Deserializer};

/// With `#[serde(default)]` on the field: omitted -> None, null -> Some(None),
/// and a supplied value -> Some(Some(value)). Used for nullable update fields.
pub fn double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
