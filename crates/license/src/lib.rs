use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

const SERVICE: &str = "imgcomp";
const KEY_ACCOUNT: &str = "license_key";
const KEY_LAST_CHECK: &str = "last_check";
const REVERIFY_INTERVAL: Duration = Duration::from_secs(3 * 24 * 3600);
const OFFLINE_GRACE: Duration = Duration::from_secs(30 * 24 * 3600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
}

#[derive(Debug, Clone)]
pub struct LicenseStatus {
    pub tier: Tier,
    pub activated: bool,
    pub last_check: Option<SystemTime>,
}

pub trait KeyValidator: std::fmt::Debug {
    fn validate(&self, key: &str) -> Result<bool>;
}

#[derive(Debug)]
pub struct CreemValidator {
    api_base: String,
    product_id: String,
}

impl CreemValidator {
    pub fn from_env() -> Result<Self> {
        let api_base = std::env::var("IMGCOMP_LICENSE_API")
            .context("IMGCOMP_LICENSE_API not set (Creem validate endpoint)")?;
        let product_id =
            std::env::var("IMGCOMP_PRODUCT_ID").context("IMGCOMP_PRODUCT_ID not set")?;
        Ok(Self {
            api_base,
            product_id,
        })
    }

    /// Always-rejects validator for when license API env vars are missing.
    /// Used so the CLI can still report a clean Free-tier status.
    pub fn dummy() -> Self {
        Self {
            api_base: String::new(),
            product_id: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    #[serde(default)]
    valid: bool,
    #[serde(default)]
    message: Option<String>,
}

impl KeyValidator for CreemValidator {
    fn validate(&self, key: &str) -> Result<bool> {
        let url = format!("{}/licenses/validate", self.api_base);
        let body = serde_json::json!({ "key": key, "product_id": self.product_id });
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(15)))
                .build(),
        );
        let resp: ValidateResponse = agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send_json(body)
            .context("cannot reach license server")?
            .body_mut()
            .read_json()
            .context("invalid server response")?;
        if !resp.valid {
            bail!("license rejected: {}", resp.message.unwrap_or_default());
        }
        Ok(true)
    }
}

pub struct LicenseManager<V: KeyValidator> {
    validator: V,
}

impl<V: KeyValidator> LicenseManager<V> {
    pub fn new(validator: V) -> Self {
        Self { validator }
    }

    pub fn activate(&self, key: &str) -> Result<Tier> {
        let key = key.trim();
        if key.is_empty() {
            bail!("license key is empty");
        }
        self.validator.validate(key)?;
        set_secret(KEY_ACCOUNT, key)?;
        set_secret(KEY_LAST_CHECK, &now_secs().to_string())?;
        Ok(Tier::Pro)
    }

    pub fn deactivate(&self) -> Result<()> {
        delete_secret(KEY_ACCOUNT)?;
        delete_secret(KEY_LAST_CHECK)?;
        Ok(())
    }

    pub fn status(&self) -> LicenseStatus {
        let has_key = get_secret(KEY_ACCOUNT).ok().flatten().is_some();
        let last_check = get_secret(KEY_LAST_CHECK)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u64>().ok())
            .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs));
        LicenseStatus {
            tier: if has_key { Tier::Pro } else { Tier::Free },
            activated: has_key,
            last_check,
        }
    }

    /// Called at startup. Re-verifies online every 3 days; stays valid offline
    /// for up to 30 days. Returns the effective tier.
    pub fn verify(&self) -> Result<Tier> {
        let st = self.status();
        if !st.activated {
            return Ok(Tier::Free);
        }
        let last = st.last_check.unwrap_or(SystemTime::UNIX_EPOCH);
        let age = SystemTime::now()
            .duration_since(last)
            .unwrap_or(Duration::ZERO);

        if age < REVERIFY_INTERVAL {
            return Ok(Tier::Pro);
        }
        if age > OFFLINE_GRACE {
            return Ok(Tier::Free);
        }

        let key = get_secret(KEY_ACCOUNT)?.unwrap_or_default();
        match self.validator.validate(&key) {
            Ok(true) => {
                set_secret(KEY_LAST_CHECK, &now_secs().to_string())?;
                Ok(Tier::Pro)
            }
            Ok(false) => Ok(Tier::Free),
            Err(_) => Ok(Tier::Pro),
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn get_secret(key: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, key)?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("keyring read failed: {e}")),
    }
}

fn set_secret(key: &str, value: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, key)
        .and_then(|e| e.set_password(value))
        .context("cannot write to system keyring")
}

fn delete_secret(key: &str) -> Result<()> {
    let entry = match keyring::Entry::new(SERVICE, key) {
        Ok(e) => e,
        Err(e) => return Err(anyhow::anyhow!("keyring access failed: {e}")),
    };
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("keyring delete failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockValidator {
        valid_keys: Vec<String>,
        fail: bool,
    }

    impl KeyValidator for MockValidator {
        fn validate(&self, key: &str) -> Result<bool> {
            if self.fail {
                bail!("network error");
            }
            Ok(self.valid_keys.contains(&key.to_string()))
        }
    }

    fn mgr(valid: &[&str]) -> LicenseManager<MockValidator> {
        let v = MockValidator {
            valid_keys: valid.iter().map(|s| s.to_string()).collect(),
            fail: false,
        };
        LicenseManager::new(v)
    }

    #[test]
    fn free_when_no_key() {
        let _ = mgr(&["good"]);
        // No persisted key in test env => status is Free.
        let m = mgr(&["good"]);
        let st = m.status();
        assert_eq!(st.tier, Tier::Free);
        assert!(!st.activated);
        assert_eq!(m.verify().unwrap(), Tier::Free);
    }

    #[test]
    fn rejects_empty_key() {
        let m = mgr(&["good"]);
        assert!(m.activate("   ").is_err());
    }
}
