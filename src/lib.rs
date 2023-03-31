pub mod spotify_to_query;
pub mod youtube_api;
pub mod error;
pub mod convert_query;
pub mod youtube_scraper;

use std::{ time::Duration, sync::Arc };
use tokio::sync::Mutex;
use serenity::model::channel::Message;
use poise::{ reply::ReplyHandle, async_trait };
use google_youtube3::{ YouTube, hyper::client::HttpConnector, hyper_rustls::HttpsConnector };
use rspotify::ClientCredsSpotify;
use songbird::{ input::Metadata, tracks::TrackHandle, Call };
use spotify_to_query::{ TrackData, extract_album_queries, extract_playlist_queries, extract_track_query };
use youtube_api::{ extract_playlist_video_metadata, extract_video_metadata };
use error::{ Error, LibError };
use youtube_scraper::search;

#[derive(Debug)]
pub struct GeneralError {
    description: String
}

impl std::fmt::Display for GeneralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.description.as_str())
    }
}

impl std::error::Error for GeneralError {
    fn description(&self) -> &str {
        &self.description.as_str()
    }
}

pub struct CleanupObject {
    message: Message,
    delay: Duration
}

impl CleanupObject {
    pub fn new(message: Message, delay: Duration) -> Self {
        Self { message, delay }
    }

    pub fn message(&self) -> &Message {
        &self.message
    }

    pub fn delay(&self) -> &Duration {
        &self.delay
    }
}

pub struct Data {
    pub cleanup: Mutex<Vec<CleanupObject>>,
    pub youtube_client: YouTube<HttpsConnector<HttpConnector>>,
    pub spotify_client: ClientCredsSpotify
}

impl Data {
    pub fn new(youtube_client: YouTube<HttpsConnector<HttpConnector>>, spotify_client: ClientCredsSpotify) -> Self {
        Self { cleanup: Mutex::new(Vec::new()), youtube_client, spotify_client }
    }

    pub async fn delete_after_delay<'a>(&self, reply_handle: ReplyHandle<'a>, delay: Duration) {
        if let Ok(message) = reply_handle.into_message().await {
            let mut cleanup_mutex = self.cleanup.lock().await;
            (*cleanup_mutex).push(CleanupObject::new(message, delay));
        }
    }

    pub async fn extract_youtube_video_metadata(&self, id: &str) -> Result<Metadata, Error> {
        extract_video_metadata(&self.youtube_client, id).await
    }

    pub async fn extract_youtube_playlist_metadata(&self, id: &str) -> Result<Vec<Metadata>, Error> {
        extract_playlist_video_metadata(&self.youtube_client, id).await
    }

    pub async fn extract_spotify_track_query(&self, id: &str) -> Result<TrackData, Error> {
        extract_track_query(&self.spotify_client, id).await
    }

    pub async fn extract_spotify_playlist_queries(&self, id: &str) -> Result<Vec<TrackData>, Error> {
        extract_playlist_queries(&self.spotify_client, id).await
    }

    pub async fn extract_spotify_album_queries(&self, id: &str) -> Result<Vec<TrackData>, Error> {
        extract_album_queries(&self.spotify_client, id).await
    }

    pub async fn convert_query(&self, query: &str) -> Result<Vec<songbird::input::Input>, Error> {
        convert_query::convert_query(&self, query).await
    }
}

pub type Context<'a> = poise::Context<'a, Data, Error>;

#[derive(Debug, Clone)]
pub struct MiniMetadata {
    pub title: String,
    pub duration: Duration,
    pub source_url: String
}

impl MiniMetadata {
    pub fn empty() -> Self {
        Self { title: String::new(), duration: Duration::ZERO, source_url: String::new() }
    }

    pub fn lossy_from_metadata(value: Metadata) -> Self {
        Self { title: value.title.unwrap_or("".to_owned()), duration: value.duration.unwrap_or(Duration::ZERO), source_url: value.source_url.unwrap_or("".to_owned()) }
    }
}

impl songbird::typemap::TypeMapKey for MiniMetadata {
    type Value = MiniMetadata;
}

impl TryFrom<Metadata> for MiniMetadata {
    type Error = LibError;
    fn try_from(value: Metadata) -> Result<Self, Self::Error> {
        Ok(Self { title: value.title.ok_or(missing_value!("title"))?, duration: value.duration.ok_or(missing_value!("duration"))?, source_url: value.source_url.ok_or(missing_value!("source_url"))? })
    }
}

#[async_trait]
pub trait LazyMetadataTrait {
    async fn read_lazy_metadata(&self) -> Option<MiniMetadata>;
    async fn write_lazy_metadata(&mut self, metadata: MiniMetadata);
    async fn generate_lazy_metadata(&mut self);
    fn is_lazy(&self) -> bool;
}

#[async_trait]
impl LazyMetadataTrait for TrackHandle {
    async fn read_lazy_metadata(&self) -> Option<MiniMetadata> {
        self.typemap().read().await.get::<MiniMetadata>().cloned()
    }

    async fn write_lazy_metadata(&mut self, metadata: MiniMetadata) {
        self.typemap().write().await.insert::<MiniMetadata>(metadata);
    }

    fn is_lazy(&self) -> bool {
        self.metadata().track == Some("$lazy_metadata$".to_owned())
    }

    async fn generate_lazy_metadata(&mut self) {
        if self.is_lazy() {
            if let Some(ref query) = self.metadata().title {
                let (title, video_id, duration) = search(query).await;
                let mut source_url = String::new();
                if let Some(video_id) = video_id {
                    source_url = format!("https://youtu.be/{}", video_id);
                }
                let metadata = MiniMetadata {title: title.unwrap_or(String::new()), duration: duration.unwrap_or(Duration::ZERO), source_url };
                self.write_lazy_metadata(metadata).await
            }
        }
    } 
}

pub struct MetadataEventHandler {
    pub handler: Arc<Mutex<Call>>
}

#[async_trait]
impl songbird::events::EventHandler for MetadataEventHandler {
    async fn act(&self, _: &songbird::EventContext<'_>) -> Option<songbird::Event> {
        let handler_guard = self.handler.lock().await;
        if let Some(mut track) = handler_guard.queue().current() {
            track.generate_lazy_metadata().await;
        }
        None
    }   
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
        while formatted_duration.len() > 5 {
            if let Some(stripped_formatted_duration) = formatted_duration.strip_prefix("00:") {
                formatted_duration = stripped_formatted_duration.to_owned();
            }
        }
    }
    formatted_duration
}