use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait,
    MiniMetadata,
    utils::format_duration
};
use poise::serenity_prelude::{ReactionType, MessageComponentInteraction};

use std::{
    time::Duration,
    sync::Arc
};
use serenity::builder::{CreateEmbed, CreateActionRow};
use tokio::{
    task::{spawn, JoinHandle},
    sync::Mutex
};
use songbird::{Call, tracks::{TrackHandle, LoopState}};
use serenity::utils::Color;
use futures::stream::*;

static TRACKS_PER_PAGE: u32 = 6;

#[poise::command(slash_command, prefix_command, aliases("q"), guild_only)]
pub async fn queue(ctx: Context<'_>, page: Option<u32>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
        if let Some(handler) = manager.get(guild.id) {
            let mut page = page.unwrap_or(0);
            let (queue_embed, last_page) = assemble_embed(handler.clone(), page).await;
            let mut last_page = last_page;
            let reply_handle = ctx.send(
                |msg| msg
                .allowed_mentions(|s| s.replied_user(true))
                .embed(|embed| {embed.clone_from(&queue_embed); embed})
                .components(|components| components.set_action_row(create_buttons(page, last_page)))
            ).await?;

            let mut collector = reply_handle.message().await?.await_component_interactions(ctx).timeout(Duration::from_secs(30)).author_id(ctx.author().id).build();
            
            while let Some(message_collector) = collector.next().await {
                match message_collector.data.custom_id.as_str() {
                    "prev" => {
                        page -= 1;
                        update_queue_embed(page, &mut last_page, message_collector, ctx, handler.clone()).await;
                    },
                    "next" => {
                        page += 1;
                        update_queue_embed(page, &mut last_page, message_collector, ctx, handler.clone()).await;
                    },
                    "reload" => {
                        update_queue_embed(page, &mut last_page, message_collector, ctx, handler.clone()).await;
                    }
                    _ => ()
                }
            }

            ctx.data().delete_after_delay(reply_handle, Duration::ZERO).await;
        }
    }
    Ok(())
}

pub async fn extract_track_data(track: TrackHandle, include_play_time: bool) -> (MiniMetadata, Option<Duration>) {
    let mut play_time = None;
    if include_play_time {
        play_time = track.get_info().await.map(|info| Some(info.play_time)).unwrap_or(None);
    }
    if track.is_lazy() {
        if let Some(metadata) = track.read_lazy_metadata().await {
            return (metadata, play_time);
        } else {
            return (MiniMetadata::empty(), play_time);
        }
        
    } else {
        return (MiniMetadata::lossy_from_metadata(track.metadata().clone()), play_time);
    }
}

pub async fn assemble_embed(handler: Arc<Mutex<Call>>, page: u32) -> (CreateEmbed, u32) {
    search_burst(handler.clone(), page).await;
    let handler_quard = handler.lock().await;
    let mut tracks_data: Vec<(MiniMetadata, Option<Duration>)> = vec![];
    let mut index = 0;
    let mut queue = handler_quard.queue().current_queue().into_iter().skip(1 + (TRACKS_PER_PAGE * page) as usize);
    let mut is_looping = false;
    if let Some(current_track) = handler_quard.queue().current() {
        if let Ok(info) = current_track.get_info().await {
            if let LoopState::Infinite = info.loops {
                is_looping = true;
            } else {
                is_looping = false;
            }
        }
        tracks_data.push(extract_track_data(current_track, true).await);
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

    let last_page = ((handler_quard.queue().len() as f32 / TRACKS_PER_PAGE as f32).ceil() - 1.0).max(0.0) as u32;
    (create_queue_embed(formatted_tracks, page, last_page, handler_quard.queue().len(), is_looping), last_page)
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
                if let None = cloned_track.read_lazy_metadata().await {
                    cloned_track.generate_lazy_metadata().await;
                }
            }
        }));
        index += 1;
    }
    for handle in handles {
        handle.await;
    }
}

pub fn create_queue_embed(tracks: Vec<String>, page: u32, last_page: u32, tracks_len: usize, is_looping: bool) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title("Queue").footer(|footer| footer.text(format!("Page: {}/{}  tracks: {}  loop: {}", page + 1, last_page.max(1), tracks_len, is_looping)));
    let mut next_up = String::new();
    if let Some(current_track) = tracks.first() {
        embed.field("Currently Playing:", current_track, false);
    } else {
        embed.field("Currently Playing:", "*Nothing*", false);
    }
    if tracks.len() > 1 {
        for (i, track) in tracks.iter().enumerate().skip(1) {
            next_up += format!("{}. {}\n", i + (page * TRACKS_PER_PAGE) as usize, track).as_str();
        }
    } else {
        next_up = "*Nothing*".to_string();
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
        let mut formatted_play_time = format_duration(Duration::from_secs(play_time.as_secs() % duration.as_secs()), Some(formatted_duration.len() as u32));
        formatted_play_time.push('/');
        formatted_track += formatted_play_time.as_str();
    }
    formatted_track += formatted_duration.as_str();
    formatted_track
}

pub fn create_buttons(page: u32, last_page: u32) -> CreateActionRow {
    let mut components = CreateActionRow::default();
    components.create_button(|button| button.custom_id("prev").emoji(ReactionType::Unicode("◀️".to_owned())).disabled(page == 0));
    components.create_button(|button| button.custom_id("next").emoji(ReactionType::Unicode("▶️".to_owned())).disabled(page + 1 >= last_page));
    components.create_button(|button| button.custom_id("reload").emoji(ReactionType::Unicode("🔄".to_owned())));
    components
}

pub async fn update_queue_embed(page: u32, last_page: &mut u32, message_collector: Arc<MessageComponentInteraction>, ctx: Context<'_>, handler: Arc<Mutex<Call>>) {
    let channel_id = message_collector.message.channel_id.0;
    let message_id = message_collector.message.id.0;
    if let Ok(mut message) = ctx.serenity_context().http.get_message(channel_id, message_id).await {
        let (new_queue_embed, new_last_page) = assemble_embed(handler, page).await;
        *last_page = new_last_page;
        message.edit(ctx, |f| f.set_embed(new_queue_embed).components(|components| components.set_action_row(create_buttons(page, *last_page)))).await;
        message_collector.defer(ctx).await;
    }
}