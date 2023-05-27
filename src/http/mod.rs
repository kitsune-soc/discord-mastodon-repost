use crate::{login_state::LoginState, persistence::UserStore};
use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::trace::TraceLayer;

mod routes;

#[derive(Clone)]
pub struct HttpState {
    login_state: LoginState,
    mastodon_redirect_uri: Arc<str>,
    user_store: UserStore,
}

pub fn routes(
    login_state: LoginState,
    mastodon_redirect_uri: Arc<str>,
    user_store: UserStore,
) -> Router {
    Router::new()
        .route("/oauth_callback", get(self::routes::oauth_callback::get))
        .layer(TraceLayer::new_for_http())
        .with_state(HttpState {
            login_state,
            mastodon_redirect_uri,
            user_store,
        })
}
