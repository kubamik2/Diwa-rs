use diwa_rs::{
    Context,
    error::Error,
    utils::{send_error, send_reply}
};
use songbird::tracks::LoopState;

#[poise::command(slash_command, prefix_command, rename = "loop")]
pub async fn loopc(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        if let Some(user_voice_state) = guild.voice_states.get(&ctx.author().id) {
            let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
            if let Some(handler) = manager.get(guild.id) {
                let handler_guard = handler.lock().await;

                if user_voice_state.channel_id.map(|f| f.0) != handler_guard.current_channel().map(|f| f.0) {
                    send_error(&ctx, "You're In a Different Channel").await;
                    return Ok(());
                }
                if let Some(current_track) = handler_guard.queue().current() {
                    drop(handler_guard);
                    if let Ok(info) = current_track.get_info().await {
                        match info.loops {
                            LoopState::Finite(_) => {
                                if let Err(_) = current_track.enable_loop() {
                                    send_error(&ctx, "Couldn't Enable Looping").await;
                                }
                                send_reply(&ctx, "Looping Enabled").await;
                            },
                            LoopState::Infinite => {
                                if let Err(_) = current_track.disable_loop() {
                                    send_error(&ctx, "Couldn't Disable Looping").await;
                                }
                                send_reply(&ctx, "Looping Disabled").await;
                            }
                        }
                    }
                }
                
            }
        }
    }
    Ok(())
}