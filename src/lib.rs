#![deny(missing_docs)]
#![deny(clippy::all)]
#![doc = include_str!("../README.md")]

use chrono::{NaiveDate, Utc};

use serde::{Deserialize, Serialize};

use std::error::Error;

/// Information used to display the APOD.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EarendelState {
    /// The title of the APOD.
    pub title: String,
    /// The binary representation of the image.
    pub img: Vec<u8>,
    /// The copyright string.
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

/// The manager of the Earendel functionality and state.
#[derive(Default)]
pub struct EarendelServer {
    cached_state: Option<(NaiveDate, EarendelState)>,
}

impl EarendelServer {
    /// Creates a new instance of an EarendelServer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current APOD image data. Returns an Error if the web request fails or if deserialization fails.
    pub async fn get_apod_image(&mut self) -> Result<EarendelState, Box<dyn Error>> {
        let today = Utc::now().date_naive();
        let new_state = match self.cached_state.as_ref() {
            Some((date, apod)) if date == &today => apod.to_owned(),
            Some(_) | None => Self::fetch_apod_image().await?,
        };
        self.cached_state = Some((today, new_state.to_owned()));

        Ok(new_state)
    }

    async fn fetch_apod_image() -> Result<EarendelState, Box<dyn Error>> {
        let api_url = "https://api.nasa.gov/planetary/apod";
        let api_key = std::env::var("EARENDEL_API_KEY")?;
        let request_url = [api_url, "?api_key=", &api_key].concat();

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
}
