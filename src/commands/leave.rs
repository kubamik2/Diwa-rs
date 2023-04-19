use diwa_rs::{
    Context,
    error::Error,
    utils::{send_error, send_reply}
};

#[poise::command(slash_command, prefix_command)]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        if let Some(user_voice_state) = guild.voice_states.get(&ctx.author().id) {
            let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
            if let Some(handler) = manager.get(guild.id) {
                let mut handler_guard = handler.lock().await;

                if user_voice_state.channel_id.map(|f| f.0) != handler_guard.current_channel().map(|f| f.0) {
                    send_error(&ctx, "You're In a Different Channel").await;
                    return Ok(());
                }
                
                handler_guard.leave().await?;
                drop(handler_guard);
                manager.remove(guild.id).await?;
                send_reply(&ctx, "Left The Channel").await;
            }  
        }
    }
    Ok(())
}