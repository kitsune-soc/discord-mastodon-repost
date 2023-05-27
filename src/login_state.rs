use crate::util::{ClientId, ClientSecret, MastodonInstance, OauthState, UserId};
use ahash::AHasher;
use indexmap::IndexMap;
use parking_lot::Mutex;
use rand::distributions::{Alphanumeric, DistString};
use std::{hash::BuildHasherDefault, sync::Arc};

// Prevent the inner map from getting way too large.
// This value is easily adjustable, just keep in mind that the value of this usize is pre-allocated upon initialisation.
const MAX_SIZE: usize = 1000;

pub struct LoginInfo {
    pub client_id: ClientId,
    pub client_secret: ClientSecret,
    pub mastodon_instance: MastodonInstance,
    pub user_id: UserId,
}

/// State shared between the Discord client and the HTTP server
///
/// Used to store a mapping between the OAuth2 state field and the Discord user ID.
/// Uses an arc'd mutex'd index map internally to provide simple removal of the first entry in case the max size is exhausted
#[derive(Clone)]
pub struct LoginState {
    inner: Arc<Mutex<IndexMap<OauthState, LoginInfo, BuildHasherDefault<AHasher>>>>,
}

impl LoginState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(IndexMap::with_capacity_and_hasher(
                MAX_SIZE,
                BuildHasherDefault::default(),
            ))),
        }
    }

    pub fn add(&self, user_id: LoginInfo) -> OauthState {
        let oauth_state: OauthState = Alphanumeric
            .sample_string(&mut rand::thread_rng(), 32)
            .into();

        let mut inner_state = self.inner.lock();
        if inner_state.len() >= MAX_SIZE {
            inner_state.swap_remove_index(0);
        }
        inner_state.insert(oauth_state.clone(), user_id);

        oauth_state
    }

    pub fn get_remove(&self, oauth_state: OauthState) -> Option<LoginInfo> {
        let mut inner_state = self.inner.lock();
        inner_state.remove(&oauth_state)
    }
}
