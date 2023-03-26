use rspotify::{
    model::{ PlaylistId, TrackId, PlayableItem, AlbumId },
    prelude::*,
    scopes, Credentials, OAuth, ClientCredsSpotify
};
use crate::error::Error;

#[derive(Debug)]
pub struct TrackData {
    pub title: String,
    pub artists: Vec<String>
}

impl TrackData {
    pub fn new(title: String, artists: Vec<String>) -> Self {
        Self { title, artists }
    }
}

pub async fn auth() -> Result<ClientCredsSpotify, Error> {
    let creds = Credentials::from_env().unwrap();
    OAuth::from_env(scopes!("playlist-read-private","playlist-read-collaborative","user-read-private","user-library-read")).unwrap();
    let mut spotify = ClientCredsSpotify::new(creds);
    spotify.request_token()?;
    spotify.config.token_refreshing = true;
    return Ok(spotify);
}

pub async fn extract_track_query(spotify: &ClientCredsSpotify, id: &str) -> Result<TrackData, Error> {
    let track_id = TrackId::from_id(id)?;

    let track = spotify.track(track_id)?;
    let title = track.name.to_owned();
    let mut artists = Vec::new();

    for artist in track.artists {
        artists.push(artist.name);
    }

    Ok(TrackData::new(title, artists))
}

pub async fn extract_playlist_queries(spotify: &ClientCredsSpotify, id: &str) -> Result<Vec<TrackData>, Error> {
    let playlist_id = PlaylistId::from_id(id)?;
    let mut queries: Vec<TrackData> = Vec::new();
    let mut playlist = spotify.playlist_items(playlist_id, None, None);

    while let Some(item) = playlist.next() {
        if let Some(track) = item?.track {
            if let PlayableItem::Track(track) = track {
                let title = track.name;
                let mut artists = Vec::new();

                for artist in track.artists {
                    artists.push(artist.name);
                }
                
                queries.push(TrackData::new(title, artists));
            }
        }
    }

    Ok(queries)
}

pub async fn extract_album_queries(spotify: &ClientCredsSpotify, id: &str) -> Result<Vec<TrackData>, Error> {
    let album_id = AlbumId::from_id(id)?;
    let mut queries: Vec<TrackData> = Vec::new();
    let mut album = spotify.album_track(album_id);

    while let Some(track) = album.next() {
        let track = track?;
        let title = track.name;
        let mut artists = Vec::new();

        for artist in track.artists {
            artists.push(artist.name);
        }

        queries.push(TrackData::new(title, artists));
    }

    Ok(queries)
}