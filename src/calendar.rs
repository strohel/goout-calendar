use crate::goout_api;
use icalendar::Calendar;
use reqwest::Client;
use rocket::{get, http::ContentType, request::Form, response::Content, FromForm};
use std::error::Error;

pub(in crate) type HandlerResult<T> = Result<T, Box<dyn Error>>;

#[derive(FromForm)]
pub(in crate) struct CalendarRequest {
    pub id: u64,
    pub language: String,
    pub after: Option<String>,
    pub split: bool,
}

#[get("/services/feeder/usercalendar.ics?<cal_req..>")]
pub(in crate) fn serve(cal_req: Form<CalendarRequest>) -> HandlerResult<Content<String>> {
    let client = Client::new();

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and simplicity. Disadvantage is
    // high latency of first byte served.
    let mut calendar = Calendar::new();
    for page in 1.. {
        let events_response = goout_api::fetch_page(&client, &cal_req, page)?;
        for event in goout_api::generate_events(&events_response, &cal_req)? {
            calendar.push(event);
        }

        if !events_response.has_next {
            break;
        }
    }

    Ok(Content(ContentType::Calendar, calendar.to_string()))
}
