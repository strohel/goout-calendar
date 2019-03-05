use crate::calendar::HandlerResult;
use chrono::Duration;
use icalendar::{Component, Event as IcalEvent};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write,
};

type DateTime = chrono::DateTime<chrono::FixedOffset>;

const ENDPOINT_URL: &str = "https://goout.net/services/feeder/v1/events.json";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Schedule {
    event_id: u64,
    url: String,
    cancelled: bool,
    #[serde(rename = "startISO8601")]
    start: DateTime,
    #[serde(rename = "endISO8601")]
    end: DateTime,
    hour_ignored: bool,
    is_long_term: bool, // TODO: use
    pricing: String,
    currency: String,
    timezone: String, // TODO: use
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
    schedule: Vec<Schedule>,
    venues: HashMap<u64, Venue>,
    performers: HashMap<u64, Performer>,
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

pub(in crate) fn fetch_page(
    client: &Client,
    id: u64,
    language: &str,
    after_opt: &Option<String>,
    page: u8,
) -> HandlerResult<EventsResponse> {
    let (user_str, page_str) = (&id.to_string(), &page.to_string());
    let mut params = vec![
        ("tag", "liked"),
        ("user", user_str),
        ("page", page_str),
        ("language", language),
        ("source", "goout.strohel.eu"),
    ];
    if let Some(after) = after_opt {
        params.push(("after", &after));
    }

    let mut raw_response = client.get(ENDPOINT_URL).query(&params).send()?;
    eprintln!("Retrieved {}.", raw_response.url());
    let response: EventsResponse = raw_response.json()?;
    response.error_for_status()?;
    // we call this on raw_response later, because that way we get better error message
    raw_response.error_for_status()?;
    Ok(response)
}

pub(in crate) fn generate_events(
    response: &EventsResponse,
    language: &str,
) -> HandlerResult<Vec<IcalEvent>> {
    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in response.schedule.iter() {
        let ical_event = create_ical_event(schedule, language, response)?;
        #[cfg(debug_assertions)]
        {
            eprintln!("Parsed {:?} as:", schedule);
            eprintln!("{}", ical_event.to_string());
        }
        events.push(ical_event);
    }
    Ok(events)
}

fn create_ical_event(
    schedule: &Schedule,
    language: &str,
    response: &EventsResponse,
) -> HandlerResult<IcalEvent> {
    let mut ical_event = IcalEvent::new();
    set_start_end(
        &mut ical_event,
        schedule.hour_ignored,
        schedule.start,
        schedule.end,
    );
    ical_event.add_property("URL", &schedule.url);
    set_cancelled(&mut ical_event, schedule.cancelled);

    let venue = response.venues.get(&schedule.venue_id).ok_or("No venue")?;
    ical_event.location(&format!(
        "{}, {}, {}, {}",
        venue.name, venue.address, venue.city, venue.locality.country.name
    ));
    ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

    let event = response.events.get(&schedule.event_id).ok_or("No event")?;
    set_summary(&mut ical_event, event, schedule.cancelled, language);
    set_description(&mut ical_event, schedule, event, &response.performers)?;

    Ok(ical_event)
}

fn set_start_end(ical_event: &mut IcalEvent, hour_ignored: bool, start: DateTime, end: DateTime) {
    // end date(time) is exclusive in iCalendar, but apparently inclusive in GoOut API
    let ical_end = end + Duration::seconds(1);
    if hour_ignored {
        ical_event.start_date(start.date());
        ical_event.end_date(ical_end.date());
    } else {
        ical_event.starts(start);
        ical_event.ends(ical_end);
    }
}

fn set_cancelled(ical_event: &mut IcalEvent, cancelled: bool) {
    ical_event.add_property("STATUS", if cancelled { "CANCELLED" } else { "CONFIRMED" });
}

fn set_summary(ical_event: &mut IcalEvent, event: &Event, cancelled: bool, language: &str) {
    let summary_prefix = if cancelled {
        // TODO: poor man's localisation
        match language {
            "cs" => "Zrušeno: ",
            _ => "Cancelled: ",
        }
    } else {
        ""
    };

    ical_event.summary(&format!(
        "{}{} ({})",
        summary_prefix,
        event.name,
        event
            .categories
            .values()
            .map(|c| &c.name[..]) // convert to &str, see https://stackoverflow.com/a/29026565/4345715
            .collect::<Vec<_>>()
            .join(", ")
    ));
}

fn set_description(
    ical_event: &mut IcalEvent,
    schedule: &Schedule,
    event: &Event,
    performers: &HashMap<u64, Performer>,
) -> HandlerResult<()> {
    let mut description = String::new();

    let mut performer_names = Vec::new();
    for performer_id in schedule.performer_ids.iter() {
        let performer = performers.get(&performer_id).ok_or("No performer")?;
        let mut performer_str = performer.name.to_string();
        if !performer.tags.is_empty() {
            write!(performer_str, " ({})", performer.tags.join(", "))?;
        }
        performer_names.push(performer_str);
    }
    if !performer_names.is_empty() {
        writeln!(description, "{}", performer_names.join(", "))?;
    }

    if !schedule.pricing.is_empty() {
        writeln!(description, "{} {}", schedule.currency, schedule.pricing)?;
    }

    let trimmed_text = event.text.trim();
    if !trimmed_text.is_empty() {
        writeln!(description, "\n{}\n", trimmed_text)?;
    }
    // Google Calendar ignores URL property, add it to text
    writeln!(description, "{}", schedule.url)?;
    ical_event.description(description.trim());
    Ok(())
}
