use diwa_rs::{
    Context,
    error::Error,
    utils::{send_error, send_reply}
};

#[poise::command(slash_command, prefix_command)]
pub async fn skip(ctx: Context<'_>) -> Result<(), Error> {
    skip_inner(ctx).await
}

pub async fn skip_inner(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        if let Some(user_voice_state) = guild.voice_states.get(&ctx.author().id) {
            let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
            if let Some(handler) = manager.get(guild.id) {
                let handler_guard = handler.lock().await;

                if user_voice_state.channel_id.map(|f| f.0) != handler_guard.current_channel().map(|f| f.0) {
                    send_error(&ctx, "You're In a Different Channel").await;
                    return Ok(());
                }

                handler_guard.queue().skip()?;
                drop(handler_guard);
                send_reply(&ctx, "Track Skipped").await;
            }
        }
    }
    Ok(())
}