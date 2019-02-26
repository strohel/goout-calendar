use crate::calendar::HandlerResult;
use chrono::{DateTime, FixedOffset};
use icalendar::{Component, Event as IcalEvent};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

const ENDPOINT_URL: &str = "https://goout.net/services/feeder/v1/events.json";

#[derive(Deserialize, Debug)]
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
struct NamedEntity {
    name: String,
}

#[derive(Deserialize)]
struct Locality {
    country: NamedEntity,
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
struct Performer {
    // TODO
}

#[derive(Deserialize)]
struct Event {
    name: String, // "Hudební ceny Apollo 2018",
    text: String, // Apollo Czech Music Critics Awards for ..."
    category: NamedEntity,
    tags: Vec<String>, // "Alternative/Indie", "Ambient", "Classical"
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
    #[serde(default)]
    performers: HashMap<u64, Performer>,
    #[serde(default)]
    events: HashMap<u64, Event>,
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

pub(in crate) fn generate_events(response: &EventsResponse) -> HandlerResult<Vec<IcalEvent>> {
    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in response.schedule.iter() {
        let mut ical_event = IcalEvent::new();
        ical_event.starts(schedule.start);
        ical_event.ends(schedule.end);
        ical_event.add_property("URL", &schedule.url); // TODO: Google Calendar ignores this

        let venue = response.venues.get(&schedule.venue_id).ok_or("No venue")?;
        ical_event.location(&format!(
            "{}, {}, {}, {}",
            venue.name, venue.address, venue.city, venue.locality.country.name
        ));
        ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

        let event = response.events.get(&schedule.event_id).ok_or("No event")?;
        ical_event.summary(&format!("{} ({})", event.name, event.category.name));
        ical_event.description(&event.text);

        eprintln!("Parsed {:?} as:", schedule);
        eprintln!("{}", ical_event.to_string());
        events.push(ical_event);
    }
    Ok(events)
}
