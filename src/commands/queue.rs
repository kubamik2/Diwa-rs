use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait
};

#[poise::command(slash_command, prefix_command)]
pub async fn queue(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
        if let Some(handler) = manager.get(guild.id) {
            let handler_quard = handler.lock().await;

            
        }
    }
    Ok(())
}