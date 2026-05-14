use keyring::Entry;

use crate::error::{AppError, AppResult};

const SERVICE: &str = "UnifiedMail";

pub trait CredentialStore: Send + Sync {
    fn set_password(&self, key: &str, password: &str) -> AppResult<()>;
    fn get_password(&self, key: &str) -> AppResult<String>;
    fn delete_password(&self, key: &str) -> AppResult<()>;
}

#[derive(Default)]
pub struct SystemCredentialStore;

impl CredentialStore for SystemCredentialStore {
    fn set_password(&self, key: &str, password: &str) -> AppResult<()> {
        Entry::new(SERVICE, key)
            .map_err(|err| AppError::Credential(err.to_string()))?
            .set_password(password)
            .map_err(|err| AppError::Credential(err.to_string()))
    }

    fn get_password(&self, key: &str) -> AppResult<String> {
        Entry::new(SERVICE, key)
            .map_err(|err| AppError::Credential(err.to_string()))?
            .get_password()
            .map_err(|err| AppError::Credential(err.to_string()))
    }

    fn delete_password(&self, key: &str) -> AppResult<()> {
        Entry::new(SERVICE, key)
            .map_err(|err| AppError::Credential(err.to_string()))?
            .delete_password()
            .map_err(|err| AppError::Credential(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct MockCredentialStore {
        values: Mutex<HashMap<String, String>>,
    }

    impl CredentialStore for MockCredentialStore {
        fn set_password(&self, key: &str, password: &str) -> AppResult<()> {
            self.values
                .lock()
                .unwrap()
                .insert(key.to_string(), password.to_string());
            Ok(())
        }

        fn get_password(&self, key: &str) -> AppResult<String> {
            self.values
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .ok_or_else(|| AppError::Credential("missing password".to_string()))
        }

        fn delete_password(&self, key: &str) -> AppResult<()> {
            self.values.lock().unwrap().remove(key);
            Ok(())
        }
    }

    #[test]
    fn mock_store_round_trips_passwords() {
        let store = MockCredentialStore::default();

        store.set_password("account-1", "secret").unwrap();
        assert_eq!(store.get_password("account-1").unwrap(), "secret");
        store.delete_password("account-1").unwrap();
        assert!(store.get_password("account-1").is_err());
    }
}
