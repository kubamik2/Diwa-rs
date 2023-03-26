use diwa_rs::{
    Context, different_channel,
    error::Error,
    LazyMetadataTrait,
    MetadataEventHandler
};
use songbird::{
    tracks::{create_player, TrackHandle},
    events::{Event, TrackEvent}
};
use serenity::utils::Color;

#[poise::command(slash_command, prefix_command)]
pub async fn play(ctx: Context<'_>, query: String) -> Result<(), Error> {
    let guild = ctx.guild();
    if let Some(guild) = guild {
        let user_voice_state = guild.voice_states.get(&ctx.author().id);
        if let Some(user_voice_state) = user_voice_state {
            let manager = songbird::get(&ctx.serenity_context()).await.unwrap();

            let handler = match manager.get(guild.id) {
                Some(handler) => handler,
                None => manager.join(guild.id, user_voice_state.channel_id.unwrap()).await.0
            };

            let mut handler_guard = handler.lock().await; 
            handler_guard.add_global_event(Event::Track(TrackEvent::Play), MetadataEventHandler {handler: handler.clone()});
             
            if user_voice_state.channel_id.map(|f| f.0) != handler_guard.current_channel().map(|f| f.0) {
                return Err(different_channel!().into());
            }

            let inputs = ctx.data().convert_query(&query).await?;

            let is_empty = handler_guard.queue().is_empty();

            let mut handles: Vec<TrackHandle> = vec![];

            for input in inputs {
                let (track, handle) = create_player(input);
                handles.push(handle);
                handler_guard.enqueue(track);
            }

            if is_empty {
                if let Some(mut handle) = handler_guard.queue().current() {
                    handle.generate_lazy_metadata().await;
                }
            }

            if handles.len() > 1 {
                ctx.send(
                    |msg| msg
                        .ephemeral(true)
                        .reply(true)
                        .allowed_mentions(|s| s.replied_user(true))
                        .embed(|embed| embed.title(format!("Added {} tracks", handles.len())).color(Color::PURPLE))
                ).await?;
            } else {
                if let Some(handle) = handles.get(0) {
                    let (title, source_url) = match handle.read_lazy_metadata().await {
                        Some(lazy_metadata) => {
                        (lazy_metadata.title, lazy_metadata.source_url)
                        },
                        None => {
                            let metadata = handle.metadata();
                            (metadata.title.clone().unwrap_or(String::new()), metadata.source_url.clone().unwrap_or(String::new()))
                        }
                    };
                    ctx.send(
                        |msg| msg
                            .ephemeral(true)
                            .reply(true)
                            .allowed_mentions(|s| s.replied_user(true))
                            .embed(|embed| embed.title("Added track").description(format!("[{}]({})", title, source_url)).color(Color::PURPLE))
                    ).await?;
                }
            }
        }
    }
    Ok(())
}