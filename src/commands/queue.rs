use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait,
    LazyMetadata
};
use poise::serenity_prelude::ReactionType;

use std::{
    time::Duration,
    sync::Arc
};
use serenity::builder::CreateEmbed;
use tokio::{
    task::{spawn, JoinHandle},
    sync::Mutex
};
use songbird::{Call, tracks::TrackHandle};
use serenity::utils::Color;
use futures::stream::*;

static TRACKS_PER_PAGE: u32 = 6;

#[poise::command(slash_command, prefix_command)]
pub async fn queue(ctx: Context<'_>, page: Option<u32>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
        if let Some(handler) = manager.get(guild.id) {
            ctx.defer().await;
            let mut page = page.unwrap_or(0);
            let (queue_embed, last_page) = assemble_embed(handler.clone(), page).await;
            let reply_handle = ctx.send(
                |msg| msg
                .allowed_mentions(|s| s.replied_user(true))
                .embed(|embed| {embed.clone_from(&queue_embed); embed})
                .components(|components| components.create_action_row(|row| 
                    row.create_button(|button| button.custom_id("prev").emoji(ReactionType::Unicode("◀️".to_owned())).disabled(page == 0))
                    .create_button(|button| button.custom_id("next").emoji(ReactionType::Unicode("▶️".to_owned())).disabled(page == last_page))
                ))
            ).await?;

            let collector = reply_handle.message().await?.await_component_interactions(ctx).timeout(Duration::from_secs(30)).author_id(ctx.author().id).build();
            
            collector.for_each(|message_collector| {
                let cloned_handler = handler.clone();
                async move {
                    match message_collector.data.custom_id.as_str() {
                        "prev" => {
                            page -= 1;
                            let channel_id = message_collector.message.channel_id.0;
                            let message_id = message_collector.message.id.0;
                            if let Ok(mut message) = ctx.serenity_context().http.get_message(channel_id, message_id).await {
                                let (new_queue_embed, last_page) = assemble_embed(cloned_handler, page).await;
                                message.edit(ctx, |f| f.set_embed(new_queue_embed)).await;
                                ctx.defer().await;
                            }
                        },
                        "next" => {
                            page += 1;
                            let channel_id = message_collector.message.channel_id.0;
                            let message_id = message_collector.message.id.0;
                            if let Ok(mut message) = ctx.serenity_context().http.get_message(channel_id, message_id).await {
                                let (new_queue_embed, last_page) = assemble_embed(cloned_handler, page).await;
                                message.edit(ctx, |f| f.set_embed(new_queue_embed)).await;
                                ctx.defer().await;
                            }
                        },
                        _ => ()
                    }
                }
            }).await;

            ctx.data().delete_after_delay(reply_handle, Duration::ZERO).await;
        }
    }
    Ok(())
}

pub async fn extract_track_data(track: TrackHandle, include_play_time: bool) -> (LazyMetadata, Option<Duration>) {
    let mut play_time = None;
    if include_play_time {
        play_time = track.get_info().await.map(|info| Some(info.play_time)).unwrap_or(None);
    }
    if track.is_lazy() {
        if let Some(metadata) = track.read_lazy_metadata().await {
            return (metadata, play_time);
        } else {
            return (LazyMetadata::empty(), play_time);
        }
        
    } else {
        return (LazyMetadata::lossy_from_metadata(track.metadata().clone()), play_time);
    }
}

pub async fn assemble_embed(handler: Arc<Mutex<Call>>, page: u32) -> (CreateEmbed, u32) {
    search_burst(handler.clone(), page).await;
    let handler_quard = handler.lock().await;
    let mut tracks_data: Vec<(LazyMetadata, Option<Duration>)> = vec![];
    let mut index = 0;
    let mut queue = handler_quard.queue().current_queue().into_iter().skip(1 + (TRACKS_PER_PAGE * page) as usize);
    if let Some(current) = handler_quard.queue().current() {
        tracks_data.push(extract_track_data(current, true).await);
    }
    while let Some(track) = (&mut queue).next() {
        if index == TRACKS_PER_PAGE {break;}
        tracks_data.push(extract_track_data(track, false).await);
        index += 1;
    }
    let mut formatted_tracks: Vec<String> = vec![];

    for data in tracks_data {
        formatted_tracks.push(format_track(data.0.title, data.0.source_url, data.0.duration, data.1))
    }
    let last_page = ((handler_quard.queue().len() - 1) as f32 / TRACKS_PER_PAGE as f32).ceil() as u32;
    (create_queue_embed(formatted_tracks, page, last_page, handler_quard.queue().len(), false), last_page)
}

pub async fn search_burst(handler: Arc<Mutex<Call>>, page: u32) {
    let handler_guard = handler.lock().await;

    let mut handles: Vec<JoinHandle<()>> = vec![];
    let mut index = 0;
    let mut queue = handler_guard.queue().current_queue().into_iter().skip(1 + (TRACKS_PER_PAGE * page) as usize);
    while let Some(track) = &mut queue.next() {
        if index == TRACKS_PER_PAGE {break;}
        let mut cloned_track = track.clone();
        handles.push(spawn(async move {
            if cloned_track.is_lazy() {
                cloned_track.generate_lazy_metadata().await;
            }
        }));
        index += 1;
    }
    for handle in handles {
        handle.await;
    }
}

pub fn create_queue_embed(tracks: Vec<String>, page: u32, last_page: u32, tracks_len: usize, loop_on: bool) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title("Queue").footer(|footer| footer.text(format!("Page: {}/{}  tracks: {}  loop: {}", page + 1, last_page.max(1), tracks_len, loop_on)));
    if let Some(current_track) = tracks.first() {
        embed.field("Currently Playing:", current_track, false);
    }   
    let mut next_up = String::new();
    for (i, track) in tracks.iter().enumerate().skip(1) {
        next_up += format!("{}. {}\n", i + (page * TRACKS_PER_PAGE) as usize, track).as_str();
    }
    embed.field("Next Up:", next_up, false);
    embed.color(Color::PURPLE);
    embed
}

pub fn format_track(title: String, source_url: String, duration: Duration, play_time: Option<Duration>) -> String {
    let mut formatted_track = String::new();
    formatted_track += format!("**[{}]({})** | ", title, source_url).as_str();
    let formatted_duration = format_duration(duration, None);
    if let Some(play_time) = play_time {
        let mut formatted_play_time = format_duration(play_time, Some(formatted_duration.len() as u32));
        formatted_play_time.push('/');
        formatted_track += formatted_play_time.as_str();
    }
    formatted_track += formatted_duration.as_str();
    formatted_track
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
        formatted_duration = formatted_duration.trim_start_matches("00:").to_owned();
    }
    formatted_duration
}