use crate::{calendar::CalendarRequest, error::HandlerResult};
use anyhow::{anyhow, Context};
use attohttpc;
use icalendar::Calendar;
#[cfg(test)]
use mockito;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

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

#[derive(Clone, Debug)]
struct Schedule {
    id: u64,
    event: Rc<Event>,
    url: String,
    cancelled: bool,
    start: DateTime,
    end: DateTime,
    uploaded_on: DateTime,
    hour_ignored: bool,
    is_long_term: bool,
    pricing: String,
    currency: String,
    venue: Rc<Venue>,
    performers: Vec<Rc<Performer>>,
}

#[derive(Clone, Deserialize, Debug)]
struct NamedEntity {
    name: String,
}

#[derive(Deserialize, Debug)]
struct Locality {
    country: NamedEntity,
}

#[derive(Deserialize, Debug)]
struct Venue {
    name: String,    // "MeetFactory"
    address: String, // "Ke Sklárně 15"
    city: String,    // "Praha 5"
    latitude: f64,   // 50.0533
    longitude: f64,  // 14.4082
    locality: Locality,
}

#[derive(Deserialize, Debug)]
struct Performer {
    name: String,
    tags: Vec<String>,
}

#[derive(Clone, Deserialize, Debug)]
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
struct EventsResponse {
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

fn fetch_page(cal_req: &CalendarRequest, page: u8) -> HandlerResult<EventsResponse> {
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

    let mut request = attohttpc::get(endpoint_url).params(params).try_prepare()?;
    let raw_response = request.send()?;
    if !raw_response.is_success() {
        return Err(anyhow!(
            "HTTP {} when fetching {}: {}",
            raw_response.status(),
            request.url(),
            raw_response.text().unwrap_or_default()
        ));
    }
    eprintln!("Retrieved {}.", request.url());
    let response: EventsResponse = raw_response.json()?;
    response.error_for_status()?;
    Ok(response)
}

fn reference_count_map<T>(input: HashMap<u64, T>) -> HashMap<u64, Rc<T>> {
    input.into_iter().map(|(id, value)| (id, Rc::new(value))).collect()
}

fn response_to_schedules(response: EventsResponse) -> HandlerResult<Vec<Schedule>> {
    let venue_map = reference_count_map(response.venues);
    let performer_map = reference_count_map(response.performers);
    let event_map = reference_count_map(response.events);

    let mut result = Vec::new();
    for on_wire in response.schedule {
        let venue = Rc::clone(venue_map.get(&on_wire.venue_id).with_context(|| {
            format!(
                "Venue#{} referenced by Schedule#{} not in API response.",
                on_wire.venue_id, on_wire.id
            )
        })?);
        let performers = on_wire
            .performer_ids
            .iter()
            .map(|performer_id| {
                performer_map
                    .get(performer_id)
                    .with_context(|| {
                        format!(
                            "Performer#{} referenced by Schedule#{} not in API response.",
                            performer_id, on_wire.id
                        )
                    })
                    .map(|performer_ref| Rc::clone(performer_ref))
            })
            .collect::<HandlerResult<Vec<_>>>()?;
        let event = Rc::clone(event_map.get(&on_wire.event_id).with_context(|| {
            format!(
                "Event#{} referenced by Schedule#{} not in API response.",
                on_wire.event_id, on_wire.id
            )
        })?);
        let schedule = Schedule {
            id: on_wire.id,
            event,
            url: on_wire.url,
            cancelled: on_wire.cancelled,
            start: on_wire.start,
            end: on_wire.end,
            uploaded_on: on_wire.uploaded_on,
            hour_ignored: on_wire.hour_ignored,
            is_long_term: on_wire.is_long_term,
            pricing: on_wire.pricing,
            currency: on_wire.currency,
            venue,
            performers,
        };
        result.push(schedule)
    }
    Ok(result)
}

pub(in crate) fn generate(cal_req: &CalendarRequest) -> HandlerResult<String> {
    let mut schedules = Vec::<Schedule>::new();
    for page in 1.. {
        let events_response = fetch_page(cal_req, page)?;
        let has_next = events_response.has_next;
        schedules.append(&mut response_to_schedules(events_response)?);

        if !has_next {
            break;
        }
    }

    let mut calendar = Calendar::new();
    for event in ical::generate_events(schedules, cal_req) {
        calendar.push(event);
    }

    Ok(calendar.to_string())
}
