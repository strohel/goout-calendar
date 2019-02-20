use crate::goout_api;
use reqwest::Client;
use rocket::get;
use std::error::Error;

pub(in crate) type HandlerResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
pub(in crate) struct Event {
    pub name: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub venue: String,
    pub street_address: Option<String>,
    pub address_locality: String,
}

#[get("/services/feeder/usercalendar.ics?<id>")]
pub(in crate) fn serve(id: u64) -> HandlerResult<String> {
    let client = Client::new();

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and siplicity. Disadvantage is
    // high latency of first byte served.
    let mut events = Vec::new();
    for page in 1.. {
        let json = goout_api::fetch_page(&client, id, page)?;
        let (html_str, has_next) = goout_api::parse_json_reply(&json)?;
        events.extend(goout_api::parse_events_html(html_str)?);

        if !has_next {
            break;
        }
    }

    Ok(events.iter().map(|x| format!("{:?}\n", x)).collect())
}
