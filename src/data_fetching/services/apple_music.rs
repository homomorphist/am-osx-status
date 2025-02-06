const ELEMENT: &str = r#"<meta property="og:image" content=""#;

// TODO: Use mzstatic.
pub async fn scrape_artist_image(artist_url: &str, resolution: usize) -> Result<Option<String>, reqwest::Error> {
    let res = reqwest::get(artist_url).await?;
    let text = res.text().await.expect("bad body");
    Ok(text.find(ELEMENT).map(|start| {
        let start = start + ELEMENT.len();
        let end = text[start..].find('"').unwrap() + start;
        let url: &str = &text[start..=end];
        let last_slash = url.len() - url.chars().rev().position(|c| c == '/').unwrap() - 1;
        format!("{}/{}x{}.png", &url[0..last_slash], resolution, resolution)
    }))
}
