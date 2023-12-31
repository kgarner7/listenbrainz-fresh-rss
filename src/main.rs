use reqwest::Client;
use rocket::{build, get, http::ContentType, launch, State};

use crate::listenbrainz::*;

mod listenbrainz;

#[get("/feed?<user>&<days>")]
async fn user_info(
    client: &State<Client>,
    user: String,
    days: Option<u32>,
) -> Result<(ContentType, String), String> {
    let channel = Payload::to_feed(client, user, days.unwrap_or(30)).await?;

    Ok((ContentType::XML, channel.to_string()))
}

#[launch]
fn rocket() -> _ {
    let client = Client::new();

    build()
        .manage(client)
        .mount("/", rocket::routes![user_info])
}
