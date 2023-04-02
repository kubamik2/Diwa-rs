mod commands;
mod convert_query;
mod spotify_to_query;
mod error;
mod youtube_api;

use std::env;
use dotenv::dotenv;
use diwa_rs::{Data, Context};
use serenity::prelude::*;
use songbird::SerenityInit;
use tokio::time::sleep;
use std::time::Duration;
use google_youtube3::{YouTube, oauth2, hyper::Client, hyper_rustls::HttpsConnectorBuilder};
use spotify_to_query::auth;

#[tokio::main]
async fn main() {
    dotenv().unwrap();
    let youtube_secret = oauth2::ServiceAccountKey {
        key_type: Some(env::var("YOUTUBE_KEY_TYPE").unwrap()),
        project_id: Some(env::var("YOUTUBE_PROJECT_ID").unwrap()),
        private_key_id: Some(env::var("YOUTUBE_PRIVATE_KEY_ID").unwrap()),
        private_key: env::var("YOUTUBE_PRIVATE_KEY").unwrap(),
        client_email: env::var("YOUTUBE_CLIENT_EMAIL").unwrap(),
        client_id: Some(env::var("YOUTUBE_CLIENT_ID").unwrap()),
        auth_uri: Some(env::var("YOUTUBE_AUTH_URI").unwrap()),
        token_uri: env::var("YOUTUBE_TOKEN_URI").unwrap(),
        auth_provider_x509_cert_url: Some(env::var("YOUTUBE_AUTH_PROVIDER_X509_CERT_URL").unwrap()),
        client_x509_cert_url: Some(env::var("YOUTUBE_CLIENT_X509_CERT_URL").unwrap())
    };

    let youtube_auth = oauth2::ServiceAccountAuthenticator::builder(youtube_secret).build().await.unwrap();

    let youtube_client = YouTube::new(Client::builder().build(HttpsConnectorBuilder::new().with_native_roots().https_or_http().enable_http1().enable_http2().build()), youtube_auth);

    let spotify_client = auth().await.unwrap();

    let token = env::var("DISCORD_TOKEN_TESTS").unwrap();
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT
                                | GatewayIntents::GUILD_VOICE_STATES | GatewayIntents::GUILD_MEMBERS
                                | GatewayIntents::DIRECT_MESSAGES | GatewayIntents::GUILD_PRESENCES
                                | GatewayIntents::GUILDS;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions { 
            commands: vec![
                commands::play::play(),
                commands::song::song(),
                commands::leave::leave(),
                commands::skip::skip(),
                commands::queue::queue(),
                commands::loopc::loopc(),
                commands::pause::pause(),
                commands::resume::resume(),
                commands::stop::stop()
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("-".to_owned()),
                ..Default::default()
            },
            post_command: |ctx| Box::pin(post_command(ctx)),
            ..Default::default()
        })
        .token(token)
        .intents(intents)
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                println!("{} Has Connected To Discord", ready.user.tag());
                poise::builtins::register_in_guild(&ctx.http, &framework.options().commands, serenity::model::id::GuildId(883721114604404757)).await?;
                Ok(Data::new(youtube_client, spotify_client))
            })
        })
        .client_settings(|client_settings| client_settings.register_songbird()
        
    );
    framework.run().await.unwrap();
}

async fn post_command<'a>(ctx: Context<'a>) {
    let mut cleanup_guard = ctx.data().cleanup.lock().await;
    let cleanup = &mut *cleanup_guard;
    sleep(Duration::from_secs(5)).await;
    for cleanup_object in cleanup.iter_mut() {
        let _ = cleanup_object.message().delete(&ctx.serenity_context().http).await;
    }
    cleanup.clear();
}