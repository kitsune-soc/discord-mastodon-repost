use crate::{http::HttpState, persistence::UserInfo};
use axum::extract::{Query, State};
use mastodon_async::{registration::Registered, scopes::Scopes};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: String,
}

pub async fn get(
    State(HttpState {
        login_state,
        mastodon_redirect_uri,
        user_store,
    }): State<HttpState>,
    Query(query): Query<CallbackQuery>,
) -> Result<&'static str, &'static str> {
    let login_info = login_state
        .get_remove(query.state.into())
        .ok_or("no login info found in state")?;

    // get the access token
    let base = format!("https://{}", login_info.mastodon_instance);
    let client = Registered::from_parts(
        &base,
        login_info.client_id.as_str(),
        login_info.client_secret.as_str(),
        &mastodon_redirect_uri,
        Scopes::write_all(),
        false,
    );
    let mastodon_client = client.complete(query.code).await.map_err(|error| {
        error!(?error, "failed to complete oauth flow");
        "couldn't complete oauth flow with your mastodon instance :("
    })?;

    user_store
        .add(
            login_info.user_id,
            UserInfo {
                access_token: mastodon_client.data.token.as_ref().into(),
                instance: login_info.mastodon_instance,
            },
        )
        .map_err(|error| {
            error!(?error, "failed to store user info into persistent state");
            "couldn't store your user info on our backend :("
        })?;

    Ok("you are now logged in and can use the repost function!")
}
