#![forbid(unsafe_code)]

use crate::middleware::IdentityStore;
use chrono::Utc;
use identity::error::IdentityError;
use identity::rbac::RoleBinding;
use identity::session::Session;
use identity::storage::SqliteIdentityStore;
use identity::types::{SessionId, UserId};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SqliteIdentityAdapter {
    store: Arc<Mutex<SqliteIdentityStore>>,
}

impl SqliteIdentityAdapter {
    pub fn new(store: SqliteIdentityStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }
}

#[async_trait::async_trait]
impl IdentityStore for SqliteIdentityAdapter {
    async fn get_session(&self, id: SessionId) -> Result<Option<Session>, IdentityError> {
        let store = self.store.lock().await;
        store.get_session(id)
    }

    async fn update_session_touch(
        &self,
        id: SessionId,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        let store = self.store.lock().await;
        store.update_session_touch(id, now)
    }

    async fn get_user_role_bindings(
        &self,
        user_id: UserId,
    ) -> Result<Vec<RoleBinding>, IdentityError> {
        let store = self.store.lock().await;
        store.get_user_role_bindings(user_id)
    }

    async fn is_revoked(&self, session: &Session) -> Result<bool, IdentityError> {
        let store = self.store.lock().await;
        store.is_revoked(session)
    }
}
