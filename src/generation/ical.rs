use super::{DateTime, Event, EventsResponse, Performer, Schedule};
use crate::{calendar::CalendarRequest, error::HandlerResult};
use anyhow::Context;
use chrono::{Duration, Utc};
use icalendar::{Component, Event as IcalEvent};
use std::collections::HashMap;
use std::fmt::Write;

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
        if schedule.is_long_term && cal_req.split {
            let first_day_end = schedule.start.date().and_hms(23, 59, 59);
            let prefix = match &cal_req.language[..] {
                "cs" => "Začátek: ",
                _ => "Begin: ",
            };
            let info = schedule_info(schedule.start, first_day_end, prefix);
            events.push(create_ical_event(schedule, &info, cal_req, response)?);

            let last_day_start = schedule.end.date().and_hms(0, 0, 0);
            let prefix = match &cal_req.language[..] {
                "cs" => "Konec: ",
                _ => "End: ",
            };
            let info = schedule_info(last_day_start, schedule.end, prefix);
            events.push(create_ical_event(schedule, &info, cal_req, response)?);
        } else {
            let info = schedule_info(schedule.start, schedule.end, "");
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
        ical_event.starts(start.with_timezone(&Utc));
        ical_event.ends(ical_end.with_timezone(&Utc));
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
