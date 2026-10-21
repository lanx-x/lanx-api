use crate::config::read_env;
use std::{io, num::NonZeroU64, time::Duration};

pub struct SmsConfig {
    pub url: reqwest::Url,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl SmsConfig {
    pub fn from_env() -> Result<Self, io::Error> {
        let config = Self {
            url: read_env("SMS_URL", "http://127.0.0.1:9000/events")?,
            connect_timeout: Duration::from_secs(
                read_env::<NonZeroU64>("SMS_CONNECT_TIMEOUT_SECS", "3")?.get(),
            ),
            request_timeout: Duration::from_secs(
                read_env::<NonZeroU64>("SMS_REQUEST_TIMEOUT_SECS", "10")?.get(),
            ),
        };
        config.validate()?;
        Ok(config)
    }

    pub(super) fn validate(&self) -> Result<(), io::Error> {
        if !matches!(self.url.scheme(), "http" | "https") || self.url.host_str().is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SMS_URL must be an absolute HTTP(S) URL",
            ));
        }
        if self.connect_timeout.is_zero() || self.request_timeout.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "SMS_CONNECT_TIMEOUT_SECS and SMS_REQUEST_TIMEOUT_SECS must be positive",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_url_and_zero_timeouts() {
        let mut config = SmsConfig {
            url: "ftp://localhost/events".parse().unwrap(),
            connect_timeout: Duration::from_secs(3),
            request_timeout: Duration::from_secs(10),
        };
        assert!(config.validate().is_err());
        config.url = "http://localhost/events".parse().unwrap();
        assert!(config.validate().is_ok());
        config.connect_timeout = Duration::ZERO;
        assert!(config.validate().is_err());
        config.connect_timeout = Duration::from_secs(3);
        config.request_timeout = Duration::ZERO;
        assert!(config.validate().is_err());
    }
}
