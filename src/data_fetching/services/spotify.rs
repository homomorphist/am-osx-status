// use std::collections::HashMap;

// use rspotify::{model::{FullAlbum, FullArtist, FullTrack, FullTracks, Image, Page, SearchAlbums, SearchArtists, SearchTracks}, prelude::*, ClientCredsSpotify};
// use serde::Deserialize;

// // taken from internal rspotify
// pub fn build_map<'key, 'value, const N: usize>(
//     array: [(&'key str, Option<&'value str>); N],
// ) -> HashMap<&'key str, &'value str> {
//     let mut map = HashMap::with_capacity(N);
//     for (key, value) in array {
//         if let Some(value) = value {
//             map.insert(key, value);
//         }
//     }
//     map
// }

// #[derive(Deserialize)]
// pub struct SpotifyConfiguration {
//     pub id: String,
//     pub secret: String
// }


// #[derive(Deserialize)]
// struct Output {
//     tracks: Page<FullTrack>,
//     artists: Page<FullArtist>
// }
// pub struct SpotifyDetails {
//     pub images: super::super::FoundTrackImages 
// }

// fn get_square_image(images: &[Image]) -> Option<&Image> {
//     let mut square: Vec<&Image> = images.iter().filter(|image| image.width == image.height).collect();
//     square.sort_by(|l, r| { l.width.cmp(&r.width).reverse() });
//     square.first().copied()
// }    

// pub async fn get_spotify_details(track: &apple_music::Track, spotify: &ClientCredsSpotify) -> Option<SpotifyDetails> {
//     let query = format!("{} {} {}", track.artist, track.album, track.name);
//     let payload: std::collections::HashMap<&str, &str> = build_map([
//         ("q", Some(&query)),
//         ("type", Some("artist,track")),
//         ("limit", Some("1")),
//     ]);

//     // With the query we're gonna assume the first song is the most accurate.
//     // From there, we can take the found albums and take a picture from the album that the song indicated it was.
//     // We also apply that same logic to the artist as well.

//     let result = spotify.api_get("search", &payload).await.unwrap();
//     let result: Output = serde_json::from_str(&result).unwrap();
//     let track = result.tracks.items.first()?;

//     Some(SpotifyDetails {
//         images: super::super::FoundTrackImages {
//             track: get_square_image(&track.album.images).map(|art| &art.url).cloned(),
//             artist: result.artists.items.first().and_then(|artist| get_square_image(&artist.images).map(|art| &art.url)).cloned()
//         }
//     })
// }