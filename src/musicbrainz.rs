use core::panic;
use std::mem::take;

use diesel::{insert_into, prelude::*};
use reqwest::Client;
use serde::Deserialize;
use tokio::{
    sync::{
        mpsc::{channel, Receiver, Sender as MpscSender},
        oneshot::Sender,
    },
    time::{sleep_until, Duration, Instant},
};

const MUSICBRAINZ_API: &str = "https://musicbrainz.org/ws/2/";

#[derive(Debug, Deserialize)]
pub struct CoverArtArchive {
    pub front: bool,
}

#[derive(Debug, Deserialize)]
pub struct Url {
    pub resource: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Relations {
    #[serde(rename = "target-type")]
    pub target_type: Option<String>,
    #[serde(rename = "type")]
    pub rel_type: Option<String>,
    pub url: Option<Url>,
}

#[derive(Debug, Deserialize)]
pub struct MbzRelease {
    #[serde(rename = "cover-art-archive")]
    pub cover_art_archive: Option<CoverArtArchive>,
    pub relations: Option<Vec<Relations>>,
}

pub type MbzResponse = Result<Vec<CachedRelease>, String>;
pub type MbzRequest = (Vec<String>, Sender<MbzResponse>);

pub struct MusicBrainz {
    client: Client,
    db: SqliteConnection,
    rx: Receiver<MbzRequest>,
}

#[derive(Default, Debug, Insertable, Queryable, Selectable)]
#[diesel(table_name = crate::schema::releases)]
pub struct CachedRelease {
    pub id: String,
    pub has_front: bool,
    pub urls: String,
}

pub const ZWSP: &str = "\u{200b}";

impl MusicBrainz {
    pub fn new(db: SqliteConnection) -> (MusicBrainz, MpscSender<MbzRequest>) {
        let (tx, rx) = channel::<MbzRequest>(100);

        let client = Client::builder()
            .user_agent(format!(
                "MusicBrainz Fetcher: {}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .expect("Failed to build client");

        (MusicBrainz { client, db, rx }, tx)
    }

    pub async fn listen(mut self) {
        use crate::schema::releases::dsl::*;

        'outer: while let Some((messages, tx)) = self.rx.recv().await {
            let mut mbz_releases: Vec<CachedRelease> = vec![];

            for message in messages {
                let mut search = match releases
                    .filter(id.eq(&message))
                    .limit(1)
                    .select(CachedRelease::as_select())
                    .load(&mut self.db)
                {
                    Err(error) => {
                        let _ = tx.send(Err(error.to_string()));
                        continue 'outer;
                    }
                    Ok(data) => data,
                };

                if search.len() == 1 {
                    mbz_releases.push(take(&mut search[0]));
                    continue;
                }

                let next_action = Instant::now() + Duration::from_secs(1);

                let request = match self
                    .client
                    .get(format!(
                        "{}release/{}?inc=url-rels&fmt=json",
                        MUSICBRAINZ_API, message
                    ))
                    .send()
                    .await
                {
                    Ok(req) => req,
                    Err(error) => {
                        let _ = tx.send(Err(error.to_string()));
                        continue 'outer;
                    }
                };

                let json = match request.json::<MbzRelease>().await {
                    Ok(resp) => resp,
                    Err(error) => {
                        let _ = tx.send(Err(error.to_string()));
                        sleep_until(next_action).await;
                        continue 'outer;
                    }
                };

                let front = if let Some(art) = &json.cover_art_archive {
                    art.front
                } else {
                    false
                };

                let all_urls = if let Some(relations) = &json.relations {
                    relations
                        .iter()
                        .filter_map(|item| {
                            if item.url.is_none()
                                || item.target_type.is_none()
                                || item.target_type.as_ref().unwrap() != "url"
                            {
                                return None;
                            }

                            let url = item.url.as_ref().unwrap();
                            match url.resource.as_ref() {
                                Some(resource) => match item.rel_type.as_ref() {
                                    Some(rel_type) => {
                                        Some(format!("{}{}{}", rel_type, ZWSP, resource))
                                    }
                                    None => Some(format!("Unknown type{}{}", ZWSP, resource)),
                                },
                                None => None,
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(ZWSP)
                } else {
                    String::from("")
                };

                let new_release = CachedRelease {
                    id: message.clone(),
                    has_front: front,
                    urls: all_urls,
                };

                let insert = insert_into(crate::schema::releases::table)
                    .values(&new_release)
                    .execute(&mut self.db);

                match insert {
                    Ok(rows) => {
                        assert_eq!(1, rows)
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error.to_string()));
                        sleep_until(next_action).await;
                        continue 'outer;
                    }
                }

                mbz_releases.push(new_release);
                sleep_until(next_action).await;
            }

            let _ = tx.send(Ok(mbz_releases));
        }
    }
}
