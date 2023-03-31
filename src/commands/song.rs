use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait,
    format_duration
};
use poise::ReplyHandle;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serenity::utils::Color;
use futures::stream::*;

#[poise::command(slash_command, prefix_command)]
pub async fn song(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
        if let Some(handler) = manager.get(guild.id) {
            let handler_guard = handler.lock().await;
            if handler_guard.queue().len() == 0 {return Ok(());}
            let current_track = handler_guard.queue().current();
            drop(handler_guard);
            if let Some(current_track) = current_track {
                if let Ok(current_track_state) = current_track.get_info().await {
                    if current_track.is_lazy() {
                        if let Some(metadata) = current_track.read_lazy_metadata().await {
                            let play_time = current_track_state.play_time;
                            let reply_handle = send_msg(&ctx, metadata.title, metadata.source_url, play_time, metadata.duration).await?;
                            ctx.data().delete_after_delay(reply_handle, Duration::from_secs(10)).await;
                        }
                    } else {
                        let metadata = current_track.metadata();
                        let title = metadata.title.clone().unwrap_or("Error".to_owned());
                        let source_url = metadata.source_url.clone().unwrap_or("".to_owned());
                        let duration = metadata.duration.clone().unwrap_or(Duration::ZERO);
                        let play_time = current_track_state.play_time;
                        let reply_handle = send_msg(&ctx, title, source_url, play_time, duration).await?;
                        ctx.data().delete_after_delay(reply_handle, Duration::from_secs(10)).await;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_msg<'a>(ctx: &'a Context<'_>, title: String, source_url: String, play_time: Duration, duration: Duration) -> Result<ReplyHandle<'a>, Error> {
    let formatted_duration = format_duration(duration, None);
    let formatted_play_time = format_duration(play_time, Some(formatted_duration.len() as u32));
    let reply_handle = ctx.send(
        |msg| msg
        .ephemeral(true)
        .allowed_mentions(|s| s.replied_user(true))
        .embed(|embed| embed
            .title("Currently Playing:")
            .description(format!("[{}]({}) | {}/{}", title, source_url, formatted_play_time, formatted_duration))
            .color(Color::PURPLE))).await?;
    Ok(reply_handle)
}