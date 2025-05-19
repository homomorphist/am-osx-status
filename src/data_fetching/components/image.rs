#[derive(Default, Debug)]
pub struct TrackImageUrlPack {
    pub artist: Option<String>,
    pub track: Option<String>
}
impl TrackImageUrlPack {
    pub fn none() -> Self {
        Self::default()
    }

    async fn apple_music_web_scrape_artist_image(artist_url: &str, resolution: usize) -> Result<Option<String>, reqwest::Error> {
        const ELEMENT: &str = r#"<meta property="og:image" content=""#;
        let res = reqwest::get(artist_url).await?;
        let text = res.text().await.expect("bad body");
        Ok(text.find(ELEMENT).map(|start| {
            use mzstatic::image::quality::Quality;
            let start = start + ELEMENT.len();
            let end = text[start..].find('"').expect("element did not close") + start;
            let mut url = mzstatic::image::MzStaticImage::parse(&text[start..end]).expect("bad url");
            url.parameters.quality = Some(Quality::new(resolution as u16).unwrap());
            url.to_string()
        }))
    }

    pub fn track_image_from_itunes(song: &itunes_api::Track) -> Option<String> {
        song.artwork_mzstatic().map(|mut mzstatic|{
            use mzstatic::image::quality::Quality;
            mzstatic.parameters.quality = Some(Quality::new(500).unwrap());
            mzstatic.to_string()
        }).ok()
    }
}
