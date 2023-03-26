//use std::error::Error;
use crate::{
    missing_value, url_error, Data,
    error::Error
};

use url::Url;
use songbird::{
    input::{
        Metadata, Input, ytdl_search, Codec, Container, ytdl, Restartable,
        restartable::Restart
    }
};
use poise::async_trait;
use tokio::process::Command;
use std::process::Stdio;

#[derive(Debug, Clone)]
pub enum Media {
    YouTubeVideo(String),
    YouTubePlaylist(String),
    SpotifyTrack(String),
    SpotifyPlaylist(String),
    SpotifyAlbum(String),
    Search(String)
}

pub fn extract_media(data: &Data, query: &str) -> Result<Media, Error> {
    let url = Url::parse(query.into())?;
    return Ok(match url.domain().unwrap() {
        "www.youtube.com" | "youtube.com" => {
            let video_id = url.query_pairs().into_owned().find(|p| p.0 == "v").map(|f| f.1);

            let result = if let Some(video_id) = video_id {
                Media::YouTubeVideo(video_id)
            } else if let Some(playlist_id) = url.query_pairs().into_owned().find(|p| p.0 == "list").map(|f| f.1) {
                Media::YouTubePlaylist(playlist_id)
            } else { return Err(url_error!("Incorrect Url Format").into()) };
            result
        },
        "www.youtu.be" | "youtu.be" => {
            let result = if let Some(video_id) = url.path().strip_prefix("/").map(|f| f.to_owned()) {
                Media::YouTubeVideo(video_id)
            } else { return Err(url_error!("Incorrect Url Format").into()) };
            result
        },
        "open.spotify.com" | "www.open.spotify.com" => {
            let argumets = url.path_segments().map(|f| f.collect::<Vec<&str>>()).ok_or(url_error!("Parsing Error"))?;
            let content_type = argumets.get(0);
            let id = argumets.get(1);

            if let Some(content_type) = content_type {
                if let Some(id) = id {
                    let result = match *content_type {
                        "track" => Media::SpotifyTrack((*id).to_owned()),
                        "playlist" => Media::SpotifyPlaylist((*id).to_owned()),
                        "album" => Media::SpotifyAlbum((*id).to_owned()),
                        _ => return Err(url_error!("Unknown Content Type").into())
                    };
                    return Ok(result);
                }
            }

            return Err(url_error!("Required Arguments Missing").into());
        }
        _ => Media::Search(query.to_owned())
    });
}

pub async fn convert_query(data: &Data, query: &str) -> Result<Vec<Input>, Error> {
    let media = extract_media(data, query)?;
    
    return Ok(match media {
        Media::YouTubeVideo(id) => {
            let video_metadata = data.extract_youtube_video_metadata(&id).await?;
            let restartable = Restartable::new(LazyQueued::Lazy(video_metadata), true).await?;
            vec![restartable.into()]
        },
        Media::YouTubePlaylist(id) => {
            let playlist_metadata = data.extract_youtube_playlist_metadata(&id).await?;
            let mut inputs: Vec<Input> = vec![];
            for video_metadata in playlist_metadata {
                let restartable = Restartable::new(LazyQueued::Lazy(video_metadata), true).await?;
                inputs.push(restartable.into());
            }
            inputs
        },
        Media::SpotifyTrack(id) => {
            let track_data = data.extract_spotify_track_query(&id).await?;
            vec![ytdl_search(format!("{} by {}", track_data.title, track_data.artists.join(", "))).await?]
        },
        Media::SpotifyPlaylist(id) | Media::SpotifyAlbum(id) => {
            let playlist_data = data.extract_spotify_playlist_queries(&id).await?;
            let mut inputs: Vec<Input> = vec![];

            for track_data in playlist_data {
                let restartable = Restartable::new(LazyQueued::Lazier(format!("{} by {}", track_data.title, track_data.artists.join(", "))), true).await?;
                inputs.push(restartable.into());
            }
            inputs
        }
        Media::Search(search_query) => {
            vec![Restartable::ytdl_search(search_query, true).await?.into()]
        }
    });

}

pub async fn ytdl_search_metadata(query: &str) -> Result<Metadata, Error> {
    let mut cmd = Command::new("yt-dlp");
    cmd.args::<Vec<&str>, &str>(vec![
        "-j", "--simulate", "-R", "infinite", "--no-playlist", "--ignore-config", "--no-warnings", &format!("ytsearch1:{}", query) 
    ]);
    let out = cmd.stdin(Stdio::null()).output().await?;

    let value = serde_json::from_slice::<serde_json::Value>(&out.stdout)?;

    let metadata = Metadata::from_ytdl_output(value);
    Ok(metadata)
}

pub enum LazyQueued {
    Lazy(Metadata),
    Lazier(String)
}

impl LazyQueued {
    fn new_lazy(metadata: Metadata) -> Result<Self, Error> {
        if metadata.source_url.is_none() {
            return Err(missing_value!("source_url").into());
        }
        Ok(LazyQueued::Lazy(metadata))
    }

    fn new_lazier(query: String) -> Result<Self, Error> {
        Ok(LazyQueued::Lazier(query))
    }
}

#[async_trait]
impl Restart for LazyQueued {
    async fn call_restart(&mut self, time: Option<std::time::Duration>) -> songbird::input::error::Result<Input> {
        match *self {
            LazyQueued::Lazy(ref metadata) => {
                return ytdl(metadata.source_url.clone().unwrap()).await;
            },
            LazyQueued::Lazier(ref search_query) => {
                return ytdl_search(search_query).await;
            }
        }
    }

    async fn lazy_init(&mut self) -> songbird::input::error::Result<(Option<Metadata>, Codec, Container)> {
        match *self {
            LazyQueued::Lazy(ref metadata) => {
                return Ok((Some(metadata.clone()), Codec::FloatPcm, Container::Raw));
            },
            LazyQueued::Lazier(ref search_query) => {
                let mut metadata = Metadata::default();
                metadata.channels = Some(2);
                metadata.sample_rate = Some(48000);
                metadata.title = Some(search_query.clone());
                metadata.track = Some("$lazy_metadata$".to_owned());
                return Ok((Some(metadata), Codec::FloatPcm, Container::Raw));
            }
        }
    }
}