use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait
};
use poise::ReplyHandle;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serenity::utils::Color;


#[poise::command(slash_command, prefix_command)]
pub async fn song(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
        if let Some(handler) = manager.get(guild.id) {
            let handler_guard = handler.lock().await;
            if handler_guard.queue().len() == 0 {return Ok(());}
            if let Some(current_track) = handler_guard.queue().current() {
                if let Ok(current_track_state) = current_track.get_info().await {
                    drop(handler_guard);
                    if current_track.is_lazy() {
                        if let Some(metadata) = current_track.read_lazy_metadata().await {
                            let play_time = current_track_state.play_time;
                            let reply_handle = send_msg(&ctx, metadata.title, metadata.source_url, play_time, metadata.duration).await?;
                            let r = reply_handle.message().await.unwrap().await_component_interaction(ctx).author_id(ctx.author().id).await.unwrap();
                            if r.data.custom_id == "skip" {
                                crate::commands::skip::skip_inner(ctx).await;
                            }
                            ctx.data().delete_after_delay(reply_handle, Duration::from_secs(5)).await;
                        }
                    } else {
                        let metadata = current_track.metadata();
                        let title = metadata.title.clone().unwrap_or("Error".to_owned());
                        let source_url = metadata.source_url.clone().unwrap_or("".to_owned());
                        let duration = metadata.duration.clone().unwrap_or(Duration::from_secs(0));
                        let play_time = current_track_state.play_time;
                        let reply_handle = send_msg(&ctx, title, source_url, play_time, duration).await?;

                        let r = reply_handle.message().await.unwrap().await_component_interaction(ctx).author_id(ctx.author().id).await.unwrap();
                        if r.data.custom_id == "skip" {
                            crate::commands::skip::skip_inner(ctx).await;
                        }
                        
                        ctx.data().delete_after_delay(reply_handle, Duration::from_secs(5)).await;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn send_msg<'a>(ctx: &'a Context<'_>, title: String, source_url: String, play_time: Duration, duration: Duration) -> Result<ReplyHandle<'a>, Error> {
    let reply_handle = ctx.send(
        |msg| msg
        .ephemeral(true)
        .allowed_mentions(|s| s.replied_user(true))
        .embed(|embed| embed
            .title("Currently Playing:")
            .description(format!("[{}]({}) | {}/{}", title, source_url, play_time.as_secs(), duration.as_secs()))
            .color(Color::PURPLE))
            .components(|components| components
                .create_action_row(|action_row| action_row.create_button(|button| button
                    .custom_id("skip")
                    .style(poise::serenity_prelude::ButtonStyle::Primary)
                    .label("skip")
                ))
            )
    ).await?;
    Ok(reply_handle)
}