use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use serde::{de, Deserialize, Deserializer, Serializer};

pub fn deserialize_option_from_str<'de, S, D>(deserializer: D) -> Result<Option<S>, D::Error>
where
    S: FromStr,
    S::Err: Display,
    D: Deserializer<'de>,
{
    let option: Option<String> = Deserialize::deserialize(deserializer)?;
    match option {
        Some(s) => S::from_str(&s).map_err(de::Error::custom).map(Some),
        None => Ok(None),
    }
}

pub fn serialize_option_debug<T, S>(
    optional_value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Debug,
{
    match optional_value {
        Some(value) => serializer.serialize_str(&format!("{:?}", value)),
        None => serializer.serialize_none(),
    }
}
