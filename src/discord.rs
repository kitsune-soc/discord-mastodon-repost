use crate::{
    login_state::{LoginInfo, LoginState},
    persistence::UserStore,
};
use anyhow::{anyhow, Error, Result};
use async_trait::async_trait;
use futures_util::{stream::FuturesUnordered, TryStreamExt};
use mastodon_async::{
    polling_time::PollingTime, scopes::Scopes, Data, Mastodon, NewStatus, Registration,
};
use serenity::{
    http::CacheHttp,
    model::{
        prelude::{
            command::{Command, CommandOptionType, CommandType},
            interaction::{
                application_command::ApplicationCommandInteraction, Interaction,
                InteractionResponseType,
            },
            Activity, Ready,
        },
        user::OnlineStatus,
    },
    prelude::{Context, EventHandler, GatewayIntents},
    Client,
};
use std::{borrow::Cow, io, sync::Arc};
use tempfile::NamedTempFile;
use tokio::fs::File;
use tokio_util::io::StreamReader;

const LOGIN_COMMAND_NAME: &str = "login";
const LOGOUT_COMMAND_NAME: &str = "logout";
const REPOST_COMMAND_NAME: &str = "Repost to Mastodon";

struct MainHandler {
    login_state: LoginState,
    mastodon_redirect_uri: Arc<str>,
    user_store: UserStore,
}

async fn login(handler: &MainHandler, user_id: String, instance: &str) -> Result<String> {
    // Register an application
    let app = Registration::new(format!("https://{instance}"))
        .client_name(env!("CARGO_PKG_NAME"))
        .scopes(Scopes::write_all())
        .redirect_uris(handler.mastodon_redirect_uri.as_ref())
        .build()
        .await?;

    // Create login URL
    let mut authorization_url = app.authorize_url().unwrap(); // This unwrap can literally not fail. Why is this even a result?
    let (_base, client_id, client_secret, _redirect_uri, _scopes, _force_login) = app.into_parts();
    let oauth_state = handler.login_state.add(LoginInfo {
        client_id: client_id.into(),
        client_secret: client_secret.into(),
        mastodon_instance: instance.into(),
        user_id: user_id.into(),
    });
    authorization_url = format!("{authorization_url}&state={oauth_state}");

    Ok(authorization_url)
}

async fn repost(
    handler: &MainHandler,
    user_id: String,
    command: &ApplicationCommandInteraction,
) -> Result<String> {
    let user_data = handler.user_store.get(user_id.into())?;
    let mastodon_client: Mastodon = Data {
        base: Cow::Owned(format!("https://{}", user_data.instance)),
        token: Cow::Owned(user_data.access_token.into()),
        ..Data::default()
    }
    .into();

    let message = command
        .data
        .resolved
        .messages
        .iter()
        .next()
        .map(|(_id, message)| message)
        .ok_or_else(|| anyhow!("no messages in message command??"))?;

    let client = reqwest::Client::new();
    let media_ids = message
        .attachments
        .iter()
        .map(|attachment| async {
            let stream = client.get(&attachment.url).send().await?.bytes_stream();
            let mut reader = StreamReader::new(
                stream.map_err(|err| io::Error::new(io::ErrorKind::BrokenPipe, err)),
            );

            let named_tempfile = NamedTempFile::new()?;
            let mut async_file = File::from_std(named_tempfile.reopen()?);
            tokio::io::copy_buf(&mut reader, &mut async_file).await?;

            let mastodon_attachment = mastodon_client.media(named_tempfile, None).await?;
            let mastodon_attachment = mastodon_client
                .wait_for_processing(mastodon_attachment, PollingTime::default())
                .await?;

            anyhow::Ok(mastodon_attachment.id.to_string())
        })
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await?;

    let mastodon_status = mastodon_client
        .new_status(NewStatus {
            status: Some(message.content.clone()),
            media_ids: Some(media_ids),
            ..NewStatus::default()
        })
        .await?;

    Ok(mastodon_status.uri)
}

#[async_trait]
impl EventHandler for MainHandler {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            // Just defer the interaction right here. Most interactions can take a while
            command
                .create_interaction_response(ctx.http(), |resp| {
                    resp.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                        .interaction_response_data(|data| data.ephemeral(true))
                })
                .await
                .ok();

            let response_text = match command.data.name.as_str() {
                cmd if cmd == LOGIN_COMMAND_NAME => {
                    let instance = command
                        .data
                        .options
                        .get(0)
                        .expect("missing instance option")
                        .value
                        .as_ref()
                        .expect("missing value")
                        .as_str()
                        .expect("value expected to be a string");

                    match login(self, command.user.id.to_string(), instance).await {
                        Ok(auth_url) => format!("We successfully registered ourselves with your instance! Use this URL to grant us permission: {auth_url}"),
                        Err(error) => format!("We couldn't register ourselves with your Mastodon instance. Error: {error}"),
                    }
                }
                cmd if cmd == LOGOUT_COMMAND_NAME => {
                    match self.user_store.remove(command.user.id.to_string().into()) {
                        Ok(..) => "Successfully removed your login".into(),
                        Err(error) => format!("There was a problem removing your login: {error}"),
                    }
                }
                cmd if cmd == REPOST_COMMAND_NAME => {
                    match repost(self, command.user.id.to_string(), &command).await {
                        Ok(status_url) => format!("We reposted the message for you on your Mastodon account! {status_url}"),
                        Err(error) => format!("We unfortunately couldn't repost the message to your Mastodon account! Error: {error}")
                    }
                }
                cmd => {
                    debug!(cmd, "received unknown command");
                    return;
                }
            };

            command
                .create_followup_message(ctx.http(), |response| {
                    response.ephemeral(true).content(response_text)
                })
                .await
                .ok();
        }
    }

    async fn ready(&self, ctx: Context, _ready: Ready) {
        info!("connected to discord api");

        ctx.shard.set_status(OnlineStatus::Online);
        ctx.shard
            .set_activity(Some(Activity::listening("Like A Dragon soundtrack")));

        Command::create_global_application_command(ctx.http(), |command| {
            command
                .name(LOGIN_COMMAND_NAME)
                .description("Log into your Mastodon account")
                .create_option(|option| {
                    option
                        .name("instance")
                        .description("Mastodon instance you wanna log into")
                        .kind(CommandOptionType::String)
                        .required(true)
                })
        })
        .await
        .unwrap();

        Command::create_global_application_command(ctx.http(), |command| {
            command
                .name(LOGOUT_COMMAND_NAME)
                .description("Log out from your Mastodon account")
        })
        .await
        .unwrap();

        Command::create_global_application_command(ctx.http(), |command| {
            command.name(REPOST_COMMAND_NAME).kind(CommandType::Message)
        })
        .await
        .unwrap();
    }
}

pub async fn start(
    discord_token: &str,
    login_state: LoginState,
    mastodon_redirect_uri: Arc<str>,
    user_store: UserStore,
) -> Result<()> {
    let mut client = Client::builder(discord_token, GatewayIntents::empty())
        .event_handler(MainHandler {
            login_state,
            mastodon_redirect_uri,
            user_store,
        })
        .await?;

    client.start_autosharded().await.map_err(Error::from)
}
