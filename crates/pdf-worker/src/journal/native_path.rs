use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(tag = "platform", content = "units", deny_unknown_fields)]
enum Native {
    Unix(Vec<u16>),
    Windows(Vec<u16>),
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Encoded {
    Text(String),
    Native(Native),
}

pub(super) fn validate(path: &Path) -> Result<(), &'static str> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let units = path.as_os_str().as_bytes();
        if units.is_empty() || units.len() > 4096 || units.contains(&0) {
            return Err("invalid native path");
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units: Vec<_> = path.as_os_str().encode_wide().take(4097).collect();
        if units.is_empty() || units.len() > 4096 || units.contains(&0) {
            return Err("invalid native path");
        }
    }
    Ok(())
}

fn encode(path: &Path) -> Result<Encoded, &'static str> {
    validate(path)?;
    if let Some(text) = path.to_str() {
        return Ok(Encoded::Text(text.to_owned()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(Encoded::Native(Native::Unix(
            path.as_os_str()
                .as_bytes()
                .iter()
                .map(|byte| u16::from(*byte))
                .collect(),
        )))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        Ok(Encoded::Native(Native::Windows(
            path.as_os_str().encode_wide().collect(),
        )))
    }
}

fn decode(encoded: Encoded) -> Result<PathBuf, &'static str> {
    let path = match encoded {
        Encoded::Text(text) => PathBuf::from(text),
        #[cfg(unix)]
        Encoded::Native(Native::Unix(units)) => {
            use std::os::unix::ffi::OsStringExt;
            if units.len() > 4096 || units.iter().any(|unit| *unit > 255) {
                return Err("invalid Unix path units");
            }
            PathBuf::from(std::ffi::OsString::from_vec(
                units.into_iter().map(|unit| unit as u8).collect(),
            ))
        }
        #[cfg(windows)]
        Encoded::Native(Native::Windows(units)) => {
            use std::os::windows::ffi::OsStringExt;
            if units.len() > 4096 {
                return Err("invalid Windows path units");
            }
            PathBuf::from(std::ffi::OsString::from_wide(&units))
        }
        _ => return Err("journal path belongs to another operating system"),
    };
    validate(&path)?;
    Ok(path)
}

pub(super) fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
    encode(path)
        .map_err(serde::ser::Error::custom)?
        .serialize(serializer)
}
pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
    decode(Encoded::deserialize(deserializer)?).map_err(serde::de::Error::custom)
}

pub(super) mod optional {
    use super::*;
    pub fn serialize<S: Serializer>(
        path: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_deref()
            .map(encode)
            .transpose()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Option::<Encoded>::deserialize(deserializer)?
            .map(decode)
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}
