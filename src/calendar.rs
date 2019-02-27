use crate::goout_api;
use icalendar::Calendar;
use reqwest::Client;
use rocket::get;
use std::error::Error;

pub(in crate) type HandlerResult<T> = Result<T, Box<dyn Error>>;

#[get("/services/feeder/usercalendar.ics?<id>&<after>")]
pub(in crate) fn serve(id: u64, after: Option<String>) -> HandlerResult<String> {
    let client = Client::new();

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and simplicity. Disadvantage is
    // high latency of first byte served.
    let mut calendar = Calendar::new();
    for page in 1.. {
        let events_response = goout_api::fetch_page(&client, id, &after, page)?;
        for event in goout_api::generate_events(&events_response)? {
            calendar.push(event);
        }

        if !events_response.has_next {
            break;
        }
    }

    Ok(calendar.to_string())
}
