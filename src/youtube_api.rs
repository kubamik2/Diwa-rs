use crate::{
    missing_value,
    error::Error
};
use songbird::input::Metadata;
use google_youtube3::{YouTube, hyper::client::HttpConnector, hyper_rustls::HttpsConnector, api::Video};
use std::str::FromStr;

pub async fn extract_video_metadata(youtube_client: &YouTube<HttpsConnector<HttpConnector>>, id: &str) -> Result<Metadata, Error> {
    let (_, result) = youtube_client.videos()
        .list(&vec!["contentDetails".to_owned(), "snippet".to_owned()])
        .add_id(id)
        .doit().await?;

    match result.items {
        Some(videos) => {
            match videos.get(0) {
                Some(video) => {
                    return extract_metadata(video);
                },
                None => {return Err(missing_value!("items[0]").into())}
            }
        },
        None => {return Err(missing_value!("items").into())}
    }
}

pub async fn extract_playlist_video_metadata(youtube_client: &YouTube<HttpsConnector<HttpConnector>>, id: &str) -> Result<Vec<Metadata>, Error> {
    let (_, result) = youtube_client.playlist_items()
        .list(&vec![])
        .playlist_id(id)
        .max_results(50)
        .doit().await?;
    
    let playlist_items = result.items;
    match playlist_items {
        Some(playlist_items) => {
            let mut playlist_item_ids: Vec<String> = vec![];
            for playlist_item in playlist_items {
                playlist_item_ids.push(playlist_item.id.ok_or(missing_value!("playlist_item_id"))?);
            }

            let (_, result) = youtube_client.videos()
            .list(&vec!["contentDetails".to_owned(), "snippet".to_owned()])
            .add_id(&playlist_item_ids.join(","))
            .doit().await?;
            
            match result.items {
                Some(video_items) => {
                    let mut metadata_collector: Vec<Metadata> = vec![];
                    for video in video_items {
                        metadata_collector.push(extract_metadata(&video)?);
                    }
                    return Ok(metadata_collector);
                },
                None => {return Err(missing_value!("video_items").into());}
            }
        },
        None => {return Err(missing_value!("playlist_items").into())}
    }
}

fn extract_metadata(video: &Video) -> Result<Metadata, Error> {
    let content_details = video.content_details.clone().ok_or(missing_value!("contentDetails"))?;
    let snippet = video.snippet.clone().ok_or(missing_value!("snippet"))?;

    let mut metadata = Metadata::default();
    metadata.channels = Some(2);
    metadata.sample_rate = Some(48000);

    metadata.duration = Some(iso8601::Duration::from_str(&content_details.duration.ok_or(missing_value!("duration"))?)?.into());
    metadata.source_url = Some(format!("https://youtu.be/{}", video.id.clone().ok_or(missing_value!("id"))?));
    metadata.title = Some(snippet.title.clone().ok_or(missing_value!("title"))?);
    
    return Ok(metadata);
}