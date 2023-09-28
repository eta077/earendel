#![deny(missing_docs)]
#![deny(clippy::all)]
#![doc = include_str!("../README.md")]

use astro_rs::coordinates::Icrs;
use chrono::{NaiveDate, Utc};

use reqwest::header::HeaderMap;
use reqwest::header::{ACCEPT, CONTENT_TYPE};

use serde::{Deserialize, Serialize};

use tracing::instrument;

use uom::si::angle::degree;

use std::env;
use std::error::Error;

/// Information used to display the APOD.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EarendelApod {
    /// The title of the APOD.
    pub title: String,
    /// The binary representation of the image.
    pub img: Vec<u8>,
    /// The copyright string.
    pub copyright: Option<String>,
}

/// Information used to display FITS files available for the APOD.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EarendelFits {
    /// The names of the FITS files for the current page.
    pub files: Vec<String>,
    /// The current page number.
    pub page: usize,
    /// The total number of available FITS files.
    pub total_hits: usize,
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

#[derive(Debug, Serialize)]
struct MastRequestParams {
    ra: f64,
    dec: f64,
    radius: f64,
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

#[derive(Debug, Serialize)]
struct MastRequest {
    service: String,
    params: MastRequestParams,
    format: String,
    pagesize: usize,
    page: usize,
    removenullcolumns: bool,
    timeout: u32,
}

impl MastRequest {
    pub fn new(params: MastRequestParams, page: usize) -> Self {
        MastRequest {
            service: String::from("Mast.Caom.Cone"),
            params,
            format: String::from("json"),
            pagesize: 25,
            page,
            removenullcolumns: true,
            timeout: 30,
        }
    }

    pub fn to_urlencoded(&self) -> String {
        let result = serde_json::to_string(self).unwrap();

        urlencoding::encode(&result).into_owned()
    }
}

#[derive(Debug, Deserialize)]
struct MastResponse {
    status: String,
    msg: String,
    data: Vec<MastResponseEntry>,
    paging: MastResponsePaging,
}

#[derive(Debug, Deserialize)]
struct MastResponseEntry {
    #[serde(rename = "intentType")]
    intent_type: Option<String>,
    obs_collection: Option<String>,
    provenance_name: Option<String>,
    instrument_name: Option<String>,
    project: Option<String>,
    filters: Option<String>,
    wavelength_region: Option<String>,
    target_name: Option<String>,
    target_classification: Option<String>,
    obs_id: Option<String>,
    s_ra: Option<f64>,
    s_dec: Option<f64>,
    dataproduct_type: Option<String>,
    proposal_pi: Option<String>,
    calib_level: Option<i64>,
    t_min: Option<f64>,
    t_max: Option<f64>,
    t_exptime: Option<f64>,
    em_min: Option<f64>,
    em_max: Option<f64>,
    obs_title: Option<String>,
    t_obs_release: Option<f64>,
    proposal_id: Option<String>,
    proposal_type: Option<String>,
    sequence_number: Option<i64>,
    s_region: Option<String>,
    #[serde(rename = "jpegURL")]
    jpeg_url: Option<String>,
    #[serde(rename = "dataURL")]
    data_url: Option<String>,
    #[serde(rename = "dataRights")]
    data_rights: Option<String>,
    #[serde(rename = "mtFlag")]
    mt_flag: Option<bool>,
    #[serde(rename = "srcDen")]
    src_den: Option<f64>,
    distance: Option<f64>,
    #[serde(rename = "_selected_")]
    selected: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct MastResponsePaging {
    page: usize,
    #[serde(rename = "pageSize")]
    page_size: usize,
    #[serde(rename = "pagesFiltered")]
    pages_filtered: usize,
    rows: usize,
    #[serde(rename = "rowsFiltered")]
    rows_filtered: usize,
    #[serde(rename = "rowsTotal")]
    rows_total: usize,
}

/// The manager of the Earendel functionality and state.
#[derive(Default)]
pub struct EarendelServer {
    cached_state: Option<(NaiveDate, EarendelApod)>,
}

impl EarendelServer {
    /// Creates a new instance of an EarendelServer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current APOD image data. Returns an Error if the web request fails or if deserialization fails.
    pub async fn get_apod_image(&mut self) -> Result<EarendelApod, Box<dyn Error>> {
        let today = Utc::now().date_naive();
        let new_state = match self.cached_state.as_ref() {
            Some((date, apod)) if date == &today => apod.to_owned(),
            Some(_) | None => Self::fetch_apod_image().await?,
        };
        self.cached_state = Some((today, new_state.to_owned()));

        Ok(new_state)
    }

    async fn fetch_apod_image() -> Result<EarendelApod, Box<dyn Error>> {
        let api_url = "https://api.nasa.gov/planetary/apod";
        let api_key = env::var("EARENDEL_APOD_API_KEY")?;
        let request_url = [api_url, "?api_key=", &api_key].concat();

        let resp = reqwest::get(request_url).await?;
        let body = resp.text().await?;
        let apod = serde_json::from_str::<Apod>(&body)?;

        let resp = reqwest::get(apod.url.ok_or("APOD did not contain image URL")?).await?;
        let img = resp.bytes().await?;

        Ok(EarendelApod {
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
    pub async fn get_fits_for_apod(&mut self, page: usize) -> Result<EarendelFits, Box<dyn Error>> {
        let apod = self.get_apod_image().await?;
        // TODO: extract name from apod title
        let name = "NGC 4632";
        let api_url = "https://mast.stsci.edu/api/v0/invoke";

        let coords = astro_rs::coordinates::lookup_by_name(name).await?;

        let params = MastRequestParams::from(coords);
        let request = MastRequest::new(params, page);
        let encoded_request = ["request=", &request.to_urlencoded()].concat();

        let client = reqwest::Client::new();

        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        headers.insert(ACCEPT, "text/plain".parse().unwrap());

        let resp = client
            .post(api_url)
            .headers(headers)
            .body(encoded_request)
            .send()
            .await?;
        let body = resp.text().await?;
        let mast = serde_json::from_str::<MastResponse>(&body)?;

        let fits_files = mast
            .data
            .iter()
            .filter_map(|entry| {
                entry.data_url.as_ref().and_then(|file| {
                    if file.contains("fits") {
                        Some(file.to_owned())
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<String>>();

        Ok(EarendelFits {
            files: fits_files,
            page,
            total_hits: mast.paging.rows_total,
        })
    }
}
