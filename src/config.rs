use std::{env, fmt::Display, io, str::FromStr};

// Shared parsing only; each module owns its environment names and defaults.
pub(crate) fn read_env<T>(name: &str, default: &str) -> Result<T, io::Error>
where
    T: FromStr,
    T::Err: Display,
{
    parse_env(name, env::var(name), default)
}

fn parse_env<T>(
    name: &str,
    value: Result<String, env::VarError>,
    default: &str,
) -> Result<T, io::Error>
where
    T: FromStr,
    T::Err: Display,
{
    let value = match value {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => default.to_owned(),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} must be valid Unicode"),
            ));
        }
    };
    value.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {name}: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_only_when_missing() {
        assert_eq!(
            parse_env::<u64>("TEST", Err(env::VarError::NotPresent), "3").unwrap(),
            3
        );
        assert_eq!(parse_env::<u64>("TEST", Ok("7".into()), "3").unwrap(), 7);
        assert!(
            parse_env::<u64>("TEST", Ok(String::new()), "3")
                .unwrap_err()
                .to_string()
                .contains("TEST")
        );
        assert!(
            parse_env::<u64>(
                "TEST",
                Err(env::VarError::NotUnicode("invalid".into())),
                "3"
            )
            .is_err()
        );
    }
}
