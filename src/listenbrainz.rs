use reqwest::Client;
use rocket::futures::TryFutureExt;
use rss::{CategoryBuilder, Channel, GuidBuilder, ImageBuilder, Item, ItemBuilder};
use serde::Deserialize;
use tokio::sync::{mpsc::Sender, oneshot::channel};

use crate::musicbrainz::{MbzRequest, ZWSP};

#[derive(Debug, Deserialize)]
pub struct LbzRelease {
    artist_credit_name: String,
    artist_mbids: Vec<String>,
    caa_id: Option<i64>,
    caa_release_mbid: Option<String>,
    confidence: Option<i64>,
    listen_count: i64,
    release_date: String,
    release_group_mbid: String,
    release_group_primary_type: Option<String>,
    release_group_secondary_type: Option<String>,
    release_mbid: String,
    release_name: String,
    release_tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Releases {
    releases: Vec<LbzRelease>,
}

#[derive(Debug, Deserialize)]
pub struct Payload {
    payload: Releases,
}

pub struct ListenBrainz {
    client: Client,
    sender: Sender<MbzRequest>,
}

const API_URL: &str = "https://api.listenbrainz.org/1/";
const ART_URL: &str = "https://coverartarchive.org/release/";
const FRONT_URL: &str = "https://listenbrainz.org/";

impl ListenBrainz {
    pub fn new(sender: Sender<MbzRequest>) -> ListenBrainz {
        let client = Client::builder()
            .user_agent(format!(
                "ListenBrainz Fetcher: {}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .expect("Failed to build client");

        ListenBrainz { client, sender }
    }

    pub async fn to_feed(&self, user: String, days: u32) -> Result<Channel, String> {
        let url = format!("{}user/{}/fresh_releases", API_URL, user);
        let title = format!("Releases for {}", user);

        let resp = self
            .client
            .get(&url)
            .query(&[("sort", "release_date"), ("days", &days.to_string())])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let json = resp.json::<Payload>().await.map_err(|e| e.to_string())?;

        let mut chan = Channel::default();
        chan.set_title(title);
        chan.set_image(
            ImageBuilder::default()
                .link(String::from(
                    "https://listenbrainz.org/static/img/listenbrainz-logo.svg",
                ))
                .url(String::from(
                    "https://listenbrainz.org/static/img/listenbrainz-logo.svg",
                ))
                .title(String::from("ListenBrainz logo"))
                .build(),
        );
        chan.set_link(url);
        chan.set_language(String::from("en-US"));

        let releases: Vec<String> = json
            .payload
            .releases
            .iter()
            .map(|release| release.release_mbid.clone())
            .collect();

        let (tx, rx) = channel();

        self.sender
            .send((releases, tx))
            .map_err(|err| err.to_string())
            .await?;

        let resp = rx
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string())?;

        let items: Vec<Item> = json
            .payload
            .releases
            .iter()
            .enumerate()
            .map(|(idx, release)| {
                let permalink = format!("{}release/{}", FRONT_URL, release.release_mbid);

                let categories = match release.release_group_primary_type {
                    Some(ref category) => {
                        vec![CategoryBuilder::default().name(category).build()]
                    }
                    None => vec![],
                };

                let guid = GuidBuilder::default()
                    .permalink(true)
                    .value(permalink.clone())
                    .build();

                let mbz_release = &resp[idx];

                let image = if mbz_release.has_front {
                    let thumb_url = format!("{}{}/front-250", ART_URL, release.release_mbid);
                    format!(
                        r#"<img src="{}" alt="{}" width="300px" height="300px" />"#,
                        thumb_url, release.release_name
                    )
                } else {
                    "".to_string()
                };

                let urls = if mbz_release.urls.len() > 0 {
                    let all_urls = mbz_release.urls.split(ZWSP).collect::<Vec<_>>().chunks(2).map(|item| 
                    
                        format!(r#"<li>{}: <a href="{}" target="_blank" rel="noreferrer noopener">{}</a></li>"#, item[0], item[1], item[1])
                    ).collect::<Vec<_>>()
                    .join("\n");

                    if all_urls.len() > 0 {
                        format!(r#"<div>
                            <h4>Available at: </h4>
                            <ul>{}</ul>
                        </div>"#, all_urls)
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                };

                let description = Some(format!(
                    r#"<div>
                        {}
                        <h3>By {}</h3>
                        {}
                    </div>"#,
                    image, release.artist_credit_name, urls
                ));

                let full_url = format!("{}{}/front-500", ART_URL, release.release_mbid);

                let content = if release.artist_mbids.len() == 1 {
                    let artist_url = format!("{}artist/{}", FRONT_URL, release.artist_mbids[0]);

                    Some(format!(
                        r#"<div>
                            <h1><a href="{}" rel="noreferrer noopener">{}</a> </h1>
                            <h3>By <a href="{}" rel="noreferrer noopener">{}</a></h3>
                            <div><img src="{}" alt="{}" width="500px" height="500px" /></div>
                        </div>"#,
                        permalink,
                        release.release_name,
                        artist_url,
                        release.artist_credit_name,
                        full_url,
                        release.release_name,
                    ))
                } else {
                    let all_artists = release
                        .artist_mbids
                        .iter()
                        .map(|mbid| {
                            let artist_url = format!("{}artist/{}", FRONT_URL, mbid);
                            format!(
                                r#"<li><a href="{}" rel="noreferrer noopener">{}</a></li>"#,
                                artist_url, artist_url
                            )
                        })
                        .collect::<Vec<String>>()
                        .join("\n");

                    Some(format!(
                        r#"<div>
                            <h1><a href="{}" rel="noreferrer noopener">{}</a> </h1>
                            <h3>By {}</h3>
                            <div><ul>{}</ul></div>
                            <div><img src="{}" alt="{}" width="500px" height="500px" /></div>
                        </div>"#,
                        permalink,
                        release.release_name,
                        release.artist_credit_name,
                        all_artists,
                        full_url,
                        release.release_name,
                    ))
                };

                ItemBuilder::default()
                    .guid(Some(guid))
                    .content(content)
                    .link(Some(permalink))
                    .title(Some(release.release_name.clone()))
                    .description(description)
                    .categories(categories)
                    .pub_date(Some(release.release_date.clone()))
                    .build()
            })
            .collect();

        chan.set_items(items);

        // channel.validate().map_err(|e| e.to_string())?;
        Ok(chan)
    }
}
