#[macro_use]
extern crate tracing;

use self::{
    login_state::LoginState,
    persistence::UserStore,
    util::{ServiceInfo, ShuttleServiceInfo},
};
use anyhow::anyhow;
use mimalloc::MiMalloc;
use shuttle_axum::ShuttleAxum;
use shuttle_persist::{Persist, PersistInstance};
use shuttle_secrets::{SecretStore, Secrets};
use std::sync::Arc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod discord;
mod http;
mod login_state;
mod persistence;
mod util;

#[shuttle_runtime::main]
async fn start(
    #[Persist] persist: PersistInstance,
    #[Secrets] secrets: SecretStore,
    #[ShuttleServiceInfo] service_info: ServiceInfo,
) -> ShuttleAxum {
    let login_state = LoginState::new();
    let user_store = UserStore::new(persist);

    let mastodon_redirect_uri: Arc<str> =
        format!("https://{}.shuttleapp.rs/oauth_callback", service_info.name).into();

    {
        let login_state = login_state.clone();
        let mastodon_redirect_uri = mastodon_redirect_uri.clone();
        let user_store = user_store.clone();

        let discord_token = secrets
            .get("DISCORD_TOKEN")
            .ok_or_else(|| anyhow!("discord token missing from secrets"))?;

        tokio::spawn(async move {
            if let Err(error) = self::discord::start(
                &discord_token,
                login_state,
                mastodon_redirect_uri,
                user_store,
            )
            .await
            {
                error!(?error, "discord task errored out");
            }
        });
    }

    Ok(self::http::routes(login_state, mastodon_redirect_uri, user_store).into())
}
