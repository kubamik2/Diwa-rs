use diwa_rs::{
    Context,
    error::Error,
    utils::{send_error, send_reply}
};

#[poise::command(slash_command, prefix_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        if let Some(user_voice_state) = guild.voice_states.get(&ctx.author().id) {
            let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
            if let Some(handler) = manager.get(guild.id) {
                let handler_guard = handler.lock().await;

                if user_voice_state.channel_id.map(|f| f.0) != handler_guard.current_channel().map(|f| f.0) {
                    send_error(&ctx, "You're In a Different Channel").await;
                    return Ok(());
                }
                handler_guard.queue().current().unwrap().stop();
                handler_guard.queue().modify_queue(|queue| queue.clear());
                drop(handler_guard);
            }
        }
    }
    Ok(())
}