use crate::{Context, MiniMetadata, MiniUser};
use poise::serenity_prelude::CreateEmbed;
use serenity::utils::Color;
use std::time::Duration;

pub async fn send_error(ctx: &Context<'_>, description: &str) {
    let result = ctx.send(|message| message
        .ephemeral(true)
        .reply(true)
        .allowed_mentions(|s| s.replied_user(true))
        .embed(|embed| embed.title("Error").description(description).color(Color::RED))
    ).await;
    if let Ok(reply_handle) = result {
        ctx.data().delete_after_delay(reply_handle, Duration::from_secs(10)).await;
    }
}

pub async fn send_reply(ctx: &Context<'_>, description: &str) {
    let result = ctx.send(|message| message
        .ephemeral(true)
        .reply(true)
        .allowed_mentions(|s| s.replied_user(true))
        .embed(|embed| embed.description(description).color(Color::PURPLE))
    ).await;
    if let Ok(reply_handle) = result {
        ctx.data().delete_after_delay(reply_handle, Duration::from_secs(10)).await;
    }
}

pub fn format_duration(duration: Duration, length: Option<u32>) -> String {
    let s = duration.as_secs() % 60;
    let m = duration.as_secs() / 60 % 60;
    let h = duration.as_secs() / 3600 % 24;
    let d = duration.as_secs() / 86400;
    let mut formatted_duration = format!("{:0>2}:{:0>2}:{:0>2}:{:0>2}", d, h, m, s);
    if let Some(length) = length {
        formatted_duration = formatted_duration.split_at(formatted_duration.len() - length as usize).1.to_owned();
    } else {
        while formatted_duration.len() > 5 {
            if let Some(stripped_formatted_duration) = formatted_duration.strip_prefix("00:") {
                formatted_duration = stripped_formatted_duration.to_owned();
            }
        }
    }
    formatted_duration
}

pub fn create_now_playing_embed(metadata: MiniMetadata, added_by: Option<MiniUser>) -> CreateEmbed {
    let formatted_duration = format_duration(metadata.duration, None);
    let mut embed = CreateEmbed::default();
    embed
    .title("Now Playing:")
    .description(format!("[{}]({}) | {}", metadata.title, metadata.source_url, formatted_duration))
    .color(Color::PURPLE);
    if let Some(added_by) = added_by {
        embed.author(|author| { author.url(format!("https://discordapp.com/users/{}", added_by.id)).name(added_by.name);
            if let Some(avatar_url) = added_by.avatar_url {
                author.icon_url(avatar_url);
            }
            author }
        );
    }
    embed
}