use diwa_rs::{
    Context,
    error::Error,
    LazyMetadataTrait
};

use std::{
    time::Duration,
    sync::Arc
};
use serenity::builder::CreateEmbed;
use tokio::{
    task::{spawn, JoinHandle},
    sync::Mutex
};
use songbird::Call;

#[poise::command(slash_command, prefix_command)]
pub async fn queue(ctx: Context<'_>, page: Option<u32>) -> Result<(), Error> {
    if let Some(guild) = ctx.guild() {
        let manager = songbird::get(&ctx.serenity_context()).await.unwrap();
        if let Some(handler) = manager.get(guild.id) {
            let handler_quard = handler.lock().await;

            
        }
    }
    Ok(())
}

pub async fn search_burst(handler: Arc<Mutex<Call>>, page: Option<u32>) {
    let tracks_per_page: u32 = 7;
    let handler_guard = handler.lock().await;

    let mut handles: Vec<JoinHandle<()>> = vec![];
    for track in handler_guard.queue().current_queue().iter_mut().skip(1 + (tracks_per_page * page.unwrap_or(0)) as usize) {
        track.generate_lazy_metadata();
    }
    for handle in handles {
        handle.await;
    }
}

pub async fn create_queue_embed(tracks: Vec<String>, page: u32, last_page: u32, tracks_len: u32, loop_on: bool) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title("Queue").footer(|footer| footer.text(format!("Page: {}/{}  tracks: {}  loop: {}", page, last_page, tracks_len, loop_on)));
    if let Some(current_track) = tracks.first() {
        embed.field("Currently Playing:", current_track, false);
    }   
    let mut next_up = String::new();
    for (i, track) in tracks.iter().enumerate().skip(1) {
        next_up += format!("{}. {}\n", i, track).as_str();
    }
    embed.field("Next Up:", next_up, false);
    embed
}

pub fn format_track(title: String, source_url: String, duration: Duration, play_time: Option<Duration>) -> String {
    let mut formatted_track = String::new();
    formatted_track += format!("```[{}]({})``` | ", title, source_url).as_str();
    let formatted_duration = format_duration(duration, None);
    if let Some(play_time) = play_time {
        let mut formatted_play_time = format_duration(duration, Some(formatted_duration.len() as u32));
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