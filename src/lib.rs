pub mod spotify_to_query;
pub mod youtube_api;
pub mod error;
pub mod convert_query;

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
pub struct LazyMetadata {
    pub title: String,
    pub duration: Duration,
    pub source_url: String
}

impl LazyMetadata {
    pub fn empty() -> Self {
        Self { title: String::new(), duration: Duration::ZERO, source_url: String::new() }
    }

    pub fn lossy_from_metadata(value: Metadata) -> Self {
        Self { title: value.title.unwrap_or("".to_owned()), duration: value.duration.unwrap_or(Duration::ZERO), source_url: value.source_url.unwrap_or("".to_owned()) }
    }
}

impl songbird::typemap::TypeMapKey for LazyMetadata {
    type Value = LazyMetadata;
}

impl TryFrom<Metadata> for LazyMetadata {
    type Error = LibError;
    fn try_from(value: Metadata) -> Result<Self, Self::Error> {
        Ok(Self { title: value.title.ok_or(missing_value!("title"))?, duration: value.duration.ok_or(missing_value!("duration"))?, source_url: value.source_url.ok_or(missing_value!("source_url"))? })
    }
}

#[async_trait]
pub trait LazyMetadataTrait {
    async fn read_lazy_metadata(&self) -> Option<LazyMetadata>;
    async fn write_lazy_metadata(&mut self, metadata: LazyMetadata);
    async fn generate_lazy_metadata(&mut self);
    fn is_lazy(&self) -> bool;
}

#[async_trait]
impl LazyMetadataTrait for TrackHandle {
    async fn read_lazy_metadata(&self) -> Option<LazyMetadata> {
        self.typemap().read().await.get::<LazyMetadata>().cloned()
    }

    async fn write_lazy_metadata(&mut self, metadata: LazyMetadata) {
        self.typemap().write().await.insert::<LazyMetadata>(metadata);
    }

    fn is_lazy(&self) -> bool {
        self.metadata().track == Some("$lazy_metadata$".to_owned())
    }

    async fn generate_lazy_metadata(&mut self) {
        if self.is_lazy() {
            if let Some(ref query) = self.metadata().title {
                if let Ok(metadata) = convert_query::ytdl_search_metadata(&query).await {
                    if let Ok(lazy_metadata) = LazyMetadata::try_from(metadata) {
                        let _ = self.write_lazy_metadata(lazy_metadata).await;
                    }
                }
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