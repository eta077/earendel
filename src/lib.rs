use serde::{Deserialize, Serialize};

use std::error::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EarendelState {
    pub title: String,
    pub img: Vec<u8>,
    pub copyright: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Apod {
    id: Option<u32>,
    copyright: Option<String>,
    date: String,
    explanation: Option<String>,
    hdurl: Option<String>,
    media_type: String,
    service_version: Option<String>,
    title: String,
    url: Option<String>,
}

pub async fn get_apod_image() -> Result<EarendelState, Box<dyn Error>> {
    let api_url = "https://api.nasa.gov/planetary/apod";
    let api_key = "LAUIiXOx3ZYhgclA1N9x3X5BA6dSYFXKzmfwfGba";
    let request_url = [api_url, "?api_key=", api_key].concat();

    let resp = reqwest::get(request_url).await?;
    let body = resp.text().await?;
    let apod = serde_json::from_str::<Apod>(&body)?;

    let resp = reqwest::get(apod.url.ok_or("APOD did not contain image URL")?).await?;
    let img = resp.bytes().await?;

    Ok(EarendelState {
        title: apod.title,
        img: img.to_vec(),
        copyright: apod.copyright,
    })
}
