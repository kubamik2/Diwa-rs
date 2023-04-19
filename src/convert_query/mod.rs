
mod youtube_media;
use crate::{
    missing_value, url_error, Data,
    error::Error
};
use youtube_media::YoutubeStreamMediaSource;
use url::Url;
use songbird::{
    input::{
        Metadata, Input, ytdl_search, Codec, Container, Restartable,
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

pub fn extract_media(query: &str) -> Result<Media, Error> {
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
    let media = extract_media(query)?;
    
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
                let restartable = Restartable::new(LazyQueued::LazySoundCloudQuery(format!("{} by {}", track_data.title, track_data.artists.join(", "))), true).await?;
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

    Ok(Metadata::from_ytdl_output(value))
}

pub enum LazyQueued {
    Lazy(Metadata),
    LazySoundCloudQuery(String)
}

impl LazyQueued {
    fn new_lazy(metadata: Metadata) -> Result<Self, Error> {
        if metadata.source_url.is_none() {
            return Err(missing_value!("source_url").into());
        }
        Ok(LazyQueued::Lazy(metadata))
    }

    fn new_lazy_soundcloud_query(query: String) -> Result<Self, Error> {
        Ok(LazyQueued::LazySoundCloudQuery(query))
    }
}

#[async_trait]
impl Restart for LazyQueued {
    async fn call_restart(&mut self, _: Option<std::time::Duration>) -> songbird::input::error::Result<Input> {
        match *self {
            LazyQueued::Lazy(ref metadata) => {
                let media = YoutubeStreamMediaSource::new("zSwcTiurwwk").await.unwrap();
                let res = Input::new(true, songbird::input::Reader::Extension(Box::new(media)), songbird::input::Codec::FloatPcm, songbird::input::Container::Raw, None);
                return Ok(res);
            },
            LazyQueued::LazySoundCloudQuery(ref search_query) => {
                return ytdl_search(search_query).await;
            }
        }
    }

    async fn lazy_init(&mut self) -> songbird::input::error::Result<(Option<Metadata>, Codec, Container)> {
        match *self {
            LazyQueued::Lazy(ref metadata) => {
                return Ok((Some(metadata.clone()), Codec::FloatPcm, Container::Raw));
            },
            LazyQueued::LazySoundCloudQuery(ref search_query) => {
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
use songbird::input::children_to_reader;
fn ffmpeg(url: &str) -> songbird::input::error::Result<Input> {
    let ffmpeg_args = [
        "-f",
        "s16le",
        "-ac",
        "2",
        "-ar",
        "48000",
        "-acodec",
        "pcm_f32le",
        "-",
    ];

    let ffmpeg = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    Ok(Input::new(true, children_to_reader::<f32>(vec![ffmpeg]), Codec::FloatPcm, Container::Raw, None))
}

pub fn ytdl2(uri: &str) -> songbird::input::error::Result<Input> {
    let ytdl_args = [
        "-f",
        "webm[abr>0]/bestaudio/best",
        "-R",
        "infinite",
        "--no-playlist",
        "--ignore-config",
        "--no-warnings",
        uri,
        "-o",
        "-",
    ];

    let ffmpeg_args = [
        "-f",
        "s16le",
        "-ac",
        "2",
        "-ar",
        "48000",
        "-acodec",
        "pcm_f32le",
        "-",
    ];

    let mut youtube_dl = std::process::Command::new("yt-dlp")
        .args(&ytdl_args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let taken_stdout = youtube_dl.stdout.take().ok_or(songbird::input::error::Error::Stdout)?;

    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg("-")
        .args(&ffmpeg_args)
        .stdin(taken_stdout)
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()?;

    Ok(Input::new(
        true,
        children_to_reader::<f32>(vec![youtube_dl, ffmpeg]),
        Codec::FloatPcm,
        Container::Raw,
        None,
    ))
}

pub fn ytdl3(uri: &str) -> songbird::input::error::Result<Input> {
    let ytdl_args = [
        "-f",
        "webm[abr>0]/bestaudio/best",
        "-R",
        "infinite",
        "--no-playlist",
        "--ignore-config",
        "--no-warnings",
        uri,
        "-o",
        r"D:\temp.mp3",
    ];

    let ffmpeg_args = [
        "-i",
        r"D:\temp.mp3",
        "-f",
        "s16le",
        "-ac",
        "2",
        "-ar",
        "48000",
        "-acodec",
        "pcm_f32le",
        "-",
    ];

    let mut youtube_dl = std::process::Command::new("yt-dlp")
        .args(&ytdl_args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn().unwrap();
    let mut buf = vec![];
    youtube_dl.stderr.unwrap().read_to_end(&mut buf);
    dbg!(buf.iter().map(|f| f.clone() as char).collect::<String>());
    let mut ffmpeg = std::process::Command::new("ffmpeg")
        .args(&ffmpeg_args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn().unwrap();

    let mut stdout = ffmpeg.stdout.take().unwrap();
    use std::io::Read;
    let mut o_vec = vec![];
    stdout.read_to_end(&mut o_vec).unwrap();
    dbg!(o_vec.len());
    Ok(Input::new(
        true,
        songbird::input::Reader::from_memory(o_vec),
        Codec::FloatPcm,
        Container::Raw,
        None,
    ))
}