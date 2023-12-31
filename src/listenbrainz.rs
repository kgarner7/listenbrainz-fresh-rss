use reqwest::Client;
use rss::{CategoryBuilder, Channel, GuidBuilder, ImageBuilder, Item, ItemBuilder};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Release {
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
    releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct Payload {
    payload: Releases,
}

const API_URL: &str = "https://api.listenbrainz.org/1/";
const ART_URL: &str = "https://coverartarchive.org/release/";
const FRONT_URL: &str = "https://listenbrainz.org/";

impl Payload {
    pub async fn to_feed(client: &Client, user: String, days: u32) -> Result<Channel, String> {
        let url = format!("{}user/{}/fresh_releases", API_URL, user);
        let title = format!("Releases for {}", user);

        let resp = client
            .get(&url)
            .query(&[("sort", "release_date"), ("days", &days.to_string())])
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let json = resp.json::<Payload>().await.map_err(|e| e.to_string())?;

        let mut channel = Channel::default();
        channel.set_title(title);
        channel.set_image(
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
        channel.set_link(url);
        channel.set_language(String::from("en-US"));

        let items: Vec<Item> = json
            .payload
            .releases
            .iter()
            .map(|release| {
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

                let thumb_url = format!("{}{}/front-250", ART_URL, release.release_mbid);
                let description = Some(format!(
                    r#"<div>
                        <img src="{}" alt="{}" width="300px" height="300px" />
                        <h3>By {}</h3>
                    </div>"#,
                    thumb_url, release.release_name, release.artist_credit_name
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

        channel.set_items(items);

        // channel.validate().map_err(|e| e.to_string())?;
        Ok(channel)
    }
}
