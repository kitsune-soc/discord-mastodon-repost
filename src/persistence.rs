use crate::util::{AccessToken, MastodonInstance, UserId};
use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shuttle_persist::PersistInstance;

#[derive(Deserialize, Serialize)]
pub struct UserInfo {
    pub access_token: AccessToken,
    pub instance: MastodonInstance,
}

#[derive(Clone)]
pub struct UserStore {
    persist_instance: PersistInstance,
}

impl UserStore {
    pub fn new(persist_instance: PersistInstance) -> Self {
        Self { persist_instance }
    }

    pub fn add(&self, user_id: UserId, user_info: UserInfo) -> Result<()> {
        self.persist_instance
            .save(user_id.as_str(), user_info)
            .map_err(Error::from)
    }

    pub fn get(&self, user_id: UserId) -> Result<UserInfo> {
        self.persist_instance
            .load(user_id.as_str())
            .map_err(Error::from)
    }

    pub fn remove(&self, user_id: UserId) -> Result<()> {
        self.persist_instance
            .save(user_id.as_str(), Value::Null)
            .map_err(Error::from)
    }
}
