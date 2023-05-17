#![deny(missing_docs)]
#![deny(clippy::all)]
#![doc = include_str!("../README.md")]

use astro_rs::coordinates::Icrs;
use chrono::{DateTime, NaiveDate, Utc};

use reqwest::header::HeaderMap;
use reqwest::header::{ACCEPT, CONTENT_TYPE};

use serde::{Deserialize, Serialize};

use tracing::instrument;

use uom::si::angle::degree;

use std::env;
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

#[derive(Debug, Deserialize, Serialize)]
struct MastRequestParams {
    ra: f64,
    dec: f64,
    radius: f64,
}

impl MastRequestParams {
    pub fn to_urlencoded(&self) -> String {
        let result = [
            ("ra", &self.ra.to_string()),
            ("dec", &self.dec.to_string()),
            ("radius", &self.radius.to_string()),
        ];
        serde_urlencoded::to_string(&result).unwrap()
    }
}

impl From<Icrs> for MastRequestParams {
    fn from(value: Icrs) -> Self {
        MastRequestParams {
            ra: value.coords.ra.get::<degree>(),
            dec: value.coords.dec.get::<degree>(),
            radius: 0.2,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct MastRequest {
    service: String,
    params: MastRequestParams,
    format: String,
    pagesize: usize,
    removenullcolumns: bool,
    timeout: u32,
    cachebreaker: DateTime<Utc>,
}

impl MastRequest {
    pub fn new(params: MastRequestParams) -> Self {
        MastRequest {
            service: String::from("Mast.Caom.Cone"),
            params,
            format: String::from("json"),
            pagesize: 25,
            removenullcolumns: true,
            timeout: 30,
            cachebreaker: Utc::now(),
        }
    }

    pub fn to_urlencoded(&self) -> String {
        let result = [
            ("service", &self.service),
            ("params", &self.params.to_urlencoded()),
            ("format", &self.format),
            ("pagesize", &self.pagesize.to_string()),
            ("removenullcolumns", &self.removenullcolumns.to_string()),
            ("timeout", &self.timeout.to_string()),
            ("cachebreaker", &self.cachebreaker.to_string()),
        ];

        serde_urlencoded::to_string(&result).unwrap()
    }
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
        let api_key = env::var("EARENDEL_APOD_API_KEY")?;
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

    /// Gets FITS files for the current APOD. Returns an error if the web request fails.
    ///
    /// ```
    /// use earendel::*;
    ///
    /// # tokio_test::block_on(async {
    /// let server = EarendelServer::new();
    /// server.get_fits_for_apod().await.unwrap();
    /// # });
    /// ```
    #[instrument(skip(self))]
    pub async fn get_fits_for_apod(&self) -> Result<(), Box<dyn Error>> {
        let api_url = "https://mast.stsci.edu/api/v0/invoke";
        let api_key = env::var("EARENDEL_MAST_API_KEY")?;

        let coords = astro_rs::coordinates::lookup_by_name("NGC 1566").await?;

        let params = MastRequestParams::from(coords);
        let request = MastRequest::new(params);

        println!("{request:?}");
        //let encoded_request = ["request=", &request.to_urlencoded()].concat();
        let encoded_request =
            serde_urlencoded::to_string(&[("request", request.to_urlencoded())]).unwrap();
        println!("encoded request: {:?}", encoded_request);

        let client = reqwest::Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        headers.insert(ACCEPT, "text/plain".parse().unwrap());

        let resp = client
            .get(api_url)
            .headers(headers)
            .body(encoded_request)
            .send()
            .await?;

        println!("response: {resp:?}");

        let body = resp.text().await?;

        println!("body: {body}");

        Ok(())
    }
}
