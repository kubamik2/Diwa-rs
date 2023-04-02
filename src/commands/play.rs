use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait,
    MetadataEventHandler,
    AddedBy,
    utils::create_now_playing_embed, MiniMetadata,
    utils::format_duration,
    utils::send_error
};
use poise::serenity_prelude::CreateEmbed;
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

            handler_guard.add_global_event(Event::Track(TrackEvent::Play), MetadataEventHandler {handler: handler.clone(), channel_id: ctx.channel_id(), http: ctx.serenity_context().http.clone()});
             
            if user_voice_state.channel_id.map(|f| f.0) != handler_guard.current_channel().map(|f| f.0) {
                send_error(&ctx, "You're In a Different Channel").await;
                return Ok(());
            }
            
            if let Ok(inputs) = ctx.data().convert_query(&query).await {
                let was_empty = handler_guard.queue().is_empty();

                let mut handles: Vec<TrackHandle> = vec![];

                for input in inputs {
                    let (track, mut handle) = create_player(input);
                    handle.write_added_by(&ctx.author()).await;
                    handles.push(handle);
                    handler_guard.enqueue(track);
                }

                let mut now_playing_embed: Option<CreateEmbed> = None;
                if was_empty {
                    if let Some(mut track) = handler_guard.queue().current() {
                        track.generate_lazy_metadata().await;
                        if track.is_lazy() {
                            if let Some(metadata) = track.read_lazy_metadata().await {
                                now_playing_embed = Some(create_now_playing_embed(metadata, track.read_added_by().await));
                            }
                        } else {
                            now_playing_embed = Some(create_now_playing_embed(MiniMetadata::lossy_from_metadata(track.metadata().clone()), track.read_added_by().await));
                        }
                    }
                }
                drop(handler_guard);
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
                        let metadata = match handle.read_lazy_metadata().await {
                            Some(lazy_metadata) => {
                            lazy_metadata
                            },
                            None => {
                                let metadata = handle.metadata();
                                MiniMetadata::lossy_from_metadata(metadata.clone())
                            }
                        };
                        ctx.send(
                            |msg| msg
                                .ephemeral(true)
                                .reply(true)
                                .allowed_mentions(|s| s.replied_user(true))
                                .embed(|embed| embed.title("Added track:").description(format!("[{}]({}) | {}", metadata.title, metadata.source_url, format_duration(metadata.duration, None))).color(Color::PURPLE))
                        ).await?;
                    }
                }

                if let Some(now_playing_embed) = now_playing_embed {
                    ctx.send(|message| message.embed(|embed| {embed.clone_from(&now_playing_embed); embed})).await;
                }
            } else {
                send_error(&ctx, "Invalid Query").await;
                return Ok(());
            }
        }
    }
    Ok(())
}