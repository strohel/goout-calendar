use crate::{calendar::CalendarRequest, error::HandlerResult};
use anyhow::{anyhow, Context};
use chrono::Duration;
use icalendar::{Component, Event as IcalEvent};
#[cfg(test)]
use mockito;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Write,
};

type DateTime = chrono::DateTime<chrono::FixedOffset>;

const ENDPOINT_PATH: &str = "/services/feeder/v1/events.json";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Schedule {
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
    is_long_term: bool, // TODO: use
    pricing: String,
    // rarely, some schedules don't contain currency key, e.g. qhstd
    #[serde(default)]
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
            return Err(anyhow!("Expected message OK, got {}.", self.message));
        }
        if self.status != 200 {
            return Err(anyhow!("Expected status 200, got {}.", self.status));
        }
        Ok(())
    }
}

pub(in crate) fn fetch_page(
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

#[derive(Debug)]
struct ScheduleInfo {
    start: DateTime,
    end: DateTime,
    summary_prefix: &'static str,
}

fn schedule_info(start: DateTime, end: DateTime, summary_prefix: &'static str) -> ScheduleInfo {
    ScheduleInfo {
        start,
        end,
        summary_prefix,
    }
}

pub(in crate) fn generate_events(
    response: &EventsResponse,
    cal_req: &CalendarRequest,
) -> HandlerResult<Vec<IcalEvent>> {
    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in response.schedule.iter() {
        let mut schedule_infos = Vec::with_capacity(2);
        if schedule.is_long_term && cal_req.split {
            let first_day_end = schedule.start.date().and_hms(23, 59, 59);
            let last_day_start = schedule.end.date().and_hms(0, 0, 0);
            let begin_prefix = match &cal_req.language[..] {
                "cs" => "Začátek: ",
                _ => "Begin: ",
            };
            let end_prefix = match &cal_req.language[..] {
                "cs" => "Konec: ",
                _ => "End: ",
            };

            schedule_infos.push(schedule_info(schedule.start, first_day_end, begin_prefix));
            schedule_infos.push(schedule_info(last_day_start, schedule.end, end_prefix));
        } else {
            schedule_infos.push(schedule_info(schedule.start, schedule.end, ""));
        }

        for info in schedule_infos {
            events.push(create_ical_event(schedule, &info, cal_req, response)?);
        }
    }
    Ok(events)
}

fn create_ical_event(
    schedule: &Schedule,
    info: &ScheduleInfo,
    cal_req: &CalendarRequest,
    response: &EventsResponse,
) -> HandlerResult<IcalEvent> {
    let mut ical_event = IcalEvent::new();

    ical_event.uid(&format!(
        "{}Schedule#{}@goout.net",
        info.summary_prefix, schedule.id
    ));
    let uploaded_on_str = &schedule.uploaded_on.format("%Y%m%dT%H%M%S").to_string();
    ical_event.add_property("DTSTAMP", uploaded_on_str);

    set_start_end(&mut ical_event, schedule.hour_ignored, info.start, info.end);
    ical_event.add_property("URL", &schedule.url);
    set_cancelled(&mut ical_event, schedule.cancelled);

    let venue = response
        .venues
        .get(&schedule.venue_id)
        .context("No venue")?;
    ical_event.location(&format!(
        "{}, {}, {}, {}",
        venue.name, venue.address, venue.city, venue.locality.country.name
    ));
    ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

    let event = response
        .events
        .get(&schedule.event_id)
        .context("No event")?;
    set_summary(
        &mut ical_event,
        event,
        info.summary_prefix,
        schedule.cancelled,
        cal_req,
    );
    set_description(&mut ical_event, schedule, event, &response.performers)?;

    #[cfg(debug_assertions)]
    {
        eprintln!("Parsed {:?} {:?} as:", schedule, info);
        eprintln!("{}", ical_event.to_string());
    }

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

fn set_summary(
    ical_event: &mut IcalEvent,
    event: &Event,
    summary_prefix: &str,
    cancelled: bool,
    cal_req: &CalendarRequest,
) {
    let cancelled_prefix = if cancelled {
        // TODO: poor man's localisation
        match &cal_req.language[..] {
            "cs" => "Zrušeno: ",
            _ => "Cancelled: ",
        }
    } else {
        ""
    };

    ical_event.summary(&format!(
        "{}{}{} ({})",
        summary_prefix,
        cancelled_prefix,
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
        let performer = performers.get(&performer_id).context("No performer")?;
        let mut performer_str = performer.name.to_string();
        if !performer.tags.is_empty() {
            write!(performer_str, " ({})", performer.tags.join(", "))?;
        }
        performer_names.push(performer_str);
    }
    if !performer_names.is_empty() {
        writeln!(description, "{}", performer_names.join(", "))?;
    }

    if !schedule.currency.is_empty() && !schedule.pricing.is_empty() {
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
