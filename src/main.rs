use std::{env, sync::Arc};

use diesel::{Connection, SqliteConnection};
use musicbrainz::MusicBrainz;
use rocket::{build, get, http::ContentType, State};
use tokio::spawn;

use crate::listenbrainz::*;

mod listenbrainz;
mod musicbrainz;
mod schema;

#[get("/feed?<user>&<days>")]
async fn user_info(
    client: &State<Arc<ListenBrainz>>,
    user: String,
    days: Option<u32>,
) -> Result<(ContentType, String), String> {
    let channel = client.to_feed(user, days.unwrap_or(30)).await?;

    Ok((ContentType::XML, channel.to_string()))
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let db_path: String = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let connection = SqliteConnection::establish(&db_path).expect("Failed to connect to database");

    let (mbz, sender) = MusicBrainz::new(connection);
    let lbz = ListenBrainz::new(sender);

    spawn(async move { mbz.listen().await });

    build()
        .manage(Arc::new(lbz))
        .mount("/", rocket::routes![user_info])
        .launch()
        .await?;

    Ok(())
}
