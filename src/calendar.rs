use reqwest::{Client, Result};
use rocket::get;

const ENDPOINT_URL: &str = "https://goout.net/services/feeder/v1/events.json";

#[get("/services/feeder/usercalendar.ics?<id>")]
pub(in crate) fn serve(id: u64) -> Result<String> {
    let client = Client::new();
    let params = &[
        ("source", "strohel.eu"),
        ("user", &id.to_string()),  // TODO: seems not to actually select user?
    ];

    let result = client.get(ENDPOINT_URL)
        .query(params)
        .send()?;
    println!("Retrieved {}.", result.url());
    result.error_for_status()?
        .text()
}
