use crate::{calendar::CalendarRequest, error::HandlerResult};
use anyhow::{anyhow, Context};
use icalendar::Calendar;
#[cfg(test)]
use mockito;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

pub mod ical;

type DateTime = chrono::DateTime<chrono::FixedOffset>;

const ENDPOINT_PATH: &str = "/services/feeder/v1/events.json";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ScheduleOnWire {
    id: u64,
    event_id: u64,
    url: String,
    cancelled: bool,
    #[serde(rename = "startISO8601")]
    start: DateTime,
    #[serde(rename = "endISO8601")]
    end: DateTime,
    #[serde(rename = "uploadedOnISO8601")]
    uploaded_on: DateTime,
    hour_ignored: bool,
    is_long_term: bool,
    pricing: String,
    // rarely, some schedules don't contain currency key, e.g. qhstd
    #[serde(default)]
    currency: String,
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
    address: String, // "Ke Sklárně 15"
    city: String,    // "Praha 5"
    latitude: f64,   // 50.0533
    longitude: f64,  // 14.4082
    locality: Locality,
}

#[derive(Deserialize)]
struct Performer {
    name: String,
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct Event {
    name: String,                           // "Hudební ceny Apollo 2018"
    text: String,                           // "Apollo Czech Music Critics Awards for ..."
    categories: BTreeMap<u64, NamedEntity>, // BTreeMap because we want stable order
}

// Instruct serde to use default values for fields not present when deserializing. This is because
// error responses don't have them filled in and we want better error message than
// "failure to parse all keys in JSON".
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(in crate) struct EventsResponse {
    status: u16,
    message: Value,
    pub has_next: bool,
    schedule: Vec<ScheduleOnWire>,
    venues: HashMap<u64, Venue>,
    performers: HashMap<u64, Performer>,
    events: HashMap<u64, Event>,
}

impl EventsResponse {
    fn error_for_status(self: &Self) -> HandlerResult<()> {
        if self.message != "OK" {
            return Err(anyhow!("Expected message OK, got {}.", self.message));
        }
        if self.status != 200 {
            return Err(anyhow!("Expected status 200, got {}.", self.status));
        }
        Ok(())
    }
}

fn fetch_page(
    client: &Client,
    cal_req: &CalendarRequest,
    page: u8,
) -> HandlerResult<EventsResponse> {
    #[cfg(not(test))]
    let host = "https://goout.net";
    #[cfg(test)]
    let host = &mockito::server_url();
    let endpoint_url = &format!("{}{}", host, ENDPOINT_PATH);

    let (user_str, page_str) = (&cal_req.id.to_string(), &page.to_string());
    let mut params = vec![
        ("tag", "liked"),
        ("user", user_str),
        ("page", page_str),
        ("language", &cal_req.language),
        ("source", "goout.strohel.eu"),
    ];
    if let Some(after) = &cal_req.after {
        params.push(("after", after));
    }

    let mut raw_response = client.get(endpoint_url).query(&params).send()?;
    if let Err(e) = raw_response.error_for_status_ref() {
        return Err(e).context(raw_response.text().unwrap_or_default());
    }
    eprintln!("Retrieved {}.", raw_response.url());
    let response: EventsResponse = raw_response.json()?;
    response.error_for_status()?;
    Ok(response)
}

pub(in crate) fn generate(client: &Client, cal_req: &CalendarRequest) -> HandlerResult<String> {
    let mut calendar = Calendar::new();

    for page in 1.. {
        let events_response = fetch_page(&client, &cal_req, page)?;
        for event in ical::generate_events(&events_response, &cal_req)? {
            calendar.push(event);
        }

        if !events_response.has_next {
            break;
        }
    }

    Ok(calendar.to_string())
}
