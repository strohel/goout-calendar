use crate::calendar::{HandlerResult};
use icalendar::{Component, Event};
use reqwest::Client;
use serde::Deserialize;

const ENDPOINT_URL: &str = "https://goout.net/services/feeder/v1/events.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Schedule {
    event_id: u64,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate) struct EventsResponse {
    status: u16,
    message: String,
    // use defaults for all other fields as error responses don't have them filled in and we want
    // better error message than failure to parse all keys from JSON.
    #[serde(default)]
    pub has_next: bool,
    #[serde(default)]
    schedule: Vec<Schedule>,
}

impl EventsResponse {
    fn error_for_status(self: &Self) -> HandlerResult<()> {
        if self.message != "OK" {
            return Err(format!("Expected message OK, got {}.", self.message).into());
        }
        if self.status != 200 {
            return Err(format!("Expected status 200, got {}.", self.status).into());
        }
        Ok(())
    }
}

pub(in crate) fn fetch_page(
    client: &Client,
    id: u64,
    page: u8,
) -> HandlerResult<EventsResponse> {
    let params = &[
        ("tag", "liked"),
        ("user", &id.to_string()),
        ("page", &page.to_string()),
        ("source", "strohel.eu"),
    ];

    let mut raw_response = client
        .get(ENDPOINT_URL)
        .query(params)
        .send()?
        .error_for_status()?;
    eprintln!("Retrieved {}.", raw_response.url());
    let response: EventsResponse = raw_response.json()?;
    response.error_for_status()?;
    Ok(response)
}

pub(in crate) fn generate_events(response: &EventsResponse) -> HandlerResult<Vec<Event>> {
    let mut events: Vec<Event> = Vec::new();
    for schedule in response.schedule.iter() {
        let mut event = Event::new();
        event.summary(&format!("{}: {}", schedule.event_id, schedule.url));
        events.push(event);
    }
    Ok(events)
}
