use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::credentials::SystemCredentialStore;
use crate::error::{AppError, AppResult};
use crate::store::Store;

pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub credentials: Arc<SystemCredentialStore>,
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        let db_path = database_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self {
            store: Arc::new(Mutex::new(Store::open(db_path)?)),
            credentials: Arc::new(SystemCredentialStore::default()),
        })
    }
}

fn database_path() -> AppResult<PathBuf> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| AppError::Other("could not resolve local data directory".to_string()))?;
    Ok(base.join("Unified Mail").join("mail.sqlite3"))
}
