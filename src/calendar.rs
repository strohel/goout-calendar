use crate::goout_api;
use icalendar::{self, Calendar, Component};
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

impl Event {
    fn to_icalendar(self: &Event) -> HandlerResult<icalendar::Event> {
        eprintln!("Parsing '{:?}'...", self);
        let mut ical_event = icalendar::Event::new();
        ical_event.summary(&self.name);
//         ical_event.starts(self.start_time);
//         if let Some(end_time) = self.end_time {
//             ical_event.ends(end_time);
//         }
        // TODO: venue, address...
        eprintln!("Parsed as\n{}", ical_event.to_string());
        Ok(ical_event)
    }
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
    let mut calendar = Calendar::new();
    for page in 1.. {
        let json = goout_api::fetch_page(&client, id, page)?;
        let (html_str, has_next) = goout_api::parse_json_reply(&json)?;
        for event in goout_api::parse_events_html(html_str)? {
            calendar.push(event.to_icalendar()?);
        }

        if !has_next {
            break;
        }
    }

    Ok(calendar.to_string())
}
