use crate::calendar::HandlerResult;
use chrono::{DateTime, FixedOffset};
use icalendar::{Component, Event};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

const ENDPOINT_URL: &str = "https://goout.net/services/feeder/v1/events.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Schedule {
    event_id: u64,
    url: String,
    cancelled: bool,
    #[serde(rename = "startISO8601")]
    start: DateTime<FixedOffset>,
    #[serde(rename = "endISO8601")]
    end: DateTime<FixedOffset>,
    pricing: String,
    source_urls: Vec<String>,
    timezone: String,
    venue_id: u64,
    performer_ids: Vec<u64>,
}

#[derive(Deserialize)]
struct Country {
    name: String,
}

#[derive(Deserialize)]
struct Locality {
    country: Country,
}

#[derive(Deserialize)]
struct Venue {
    name: String,    // "MeetFactory"
    address: String, // "Ke Sklárně 15",
    city: String,    // "Praha 5",
    latitude: f64,   // 50.0533,
    longitude: f64,  // 14.4082,
    locality: Locality,
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
    #[serde(default)]
    venues: HashMap<u64, Venue>,
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

pub(in crate) fn fetch_page(client: &Client, id: u64, page: u8) -> HandlerResult<EventsResponse> {
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
        let mut ical_event = Event::new();
        ical_event.summary(&format!("{}: {}", schedule.event_id, schedule.url));
        ical_event.starts(schedule.start);
        ical_event.ends(schedule.end);

        let venue = response.venues.get(&schedule.venue_id).ok_or("No venue")?;
        ical_event.location(&format!(
            "{}, {}, {}, {}",
            venue.name, venue.address, venue.city, venue.locality.country.name
        ));
        ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

        events.push(ical_event);
    }
    Ok(events)
}
