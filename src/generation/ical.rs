use super::{DateTime, Schedule};
use crate::calendar::{CalendarRequest, LongtermHandling};
use chrono::{Duration, Utc};
use icalendar::{Component, Event as IcalEvent};
use std::rc::Rc;

pub(super) fn generate_events(
    schedules: Vec<Schedule>,
    cal_req: &CalendarRequest,
) -> Vec<IcalEvent> {
    let language: &str = &cal_req.language;

    match cal_req.longterm {
        LongtermHandling::Preserve => generate_events_preserve(schedules, language),
        LongtermHandling::Split => generate_events_split(schedules, language),
        LongtermHandling::Aggregate => unimplemented!(),
    }
}

fn generate_events_preserve(schedules: Vec<Schedule>, language: &str) -> Vec<IcalEvent> {
    schedules.iter().map(|s| create_ical_event(s, language)).collect()
}

fn generate_events_split(schedules: Vec<Schedule>, language: &str) -> Vec<IcalEvent> {
    let begin_prefix = match language {
        "cs" => "Začátek: ",
        _ => "Begin: ",
    };
    let end_prefix = match language {
        "cs" => "Konec: ",
        _ => "End: ",
    };

    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in schedules {
        if schedule.is_long_term {
            let mut first_day_schedule = schedule.clone();
            first_day_schedule.id = 1_000_000_000_000 + schedule.id;
            Rc::make_mut(&mut first_day_schedule.event).name =
                format!("{}{}", begin_prefix, schedule.event.name);
            first_day_schedule.end = schedule.start.date().and_hms(0, 0, 0) + Duration::days(1);
            events.push(create_ical_event(&first_day_schedule, language));

            let mut last_day_schedule = schedule.clone();
            last_day_schedule.id = 2_000_000_000_000 + schedule.id;
            Rc::make_mut(&mut last_day_schedule.event).name =
                format!("{}{}", end_prefix, schedule.event.name);
            last_day_schedule.start = schedule.end.date().and_hms(0, 0, 0) - Duration::days(1);
            events.push(create_ical_event(&last_day_schedule, language));
        } else {
            events.push(create_ical_event(&schedule, language));
        }
    }
    events
}

fn create_ical_event(schedule: &Schedule, language: &str) -> IcalEvent {
    let mut ical_event = IcalEvent::new();

    ical_event.uid(&format!("Schedule#{}@goout.net", schedule.id));
    let uploaded_on_str = &schedule.uploaded_on.format("%Y%m%dT%H%M%S").to_string();
    ical_event.add_property("DTSTAMP", uploaded_on_str);

    set_start_end(&mut ical_event, schedule.hour_ignored, schedule.start, schedule.end);
    ical_event.add_property("URL", &schedule.url);
    set_cancelled(&mut ical_event, schedule.cancelled);

    let venue = &schedule.venue;
    ical_event.location(&format!(
        "{}, {}, {}, {}",
        venue.name, venue.address, venue.city, venue.locality.country.name
    ));
    ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

    ical_event.summary(&get_summary(schedule, language));
    ical_event.description(&get_description(schedule));

    ical_event
}

fn set_start_end(ical_event: &mut IcalEvent, hour_ignored: bool, start: DateTime, end: DateTime) {
    if hour_ignored {
        ical_event.start_date(start.date());
        ical_event.end_date(end.date());
    } else {
        ical_event.starts(start.with_timezone(&Utc));
        ical_event.ends(end.with_timezone(&Utc));
    }
}

fn set_cancelled(ical_event: &mut IcalEvent, cancelled: bool) {
    ical_event.add_property("STATUS", if cancelled { "CANCELLED" } else { "CONFIRMED" });
}

fn get_summary(schedule: &Schedule, language: &str) -> String {
    let cancelled_prefix = if schedule.cancelled {
        // TODO: poor man's localisation
        match language {
            "cs" => "Zrušeno: ",
            _ => "Cancelled: ",
        }
    } else {
        ""
    };

    format!(
        "{}{} ({})",
        cancelled_prefix,
        schedule.event.name,
        schedule
            .event
            .categories
            .values()
            .map(|c| &c.name[..]) // convert to &str, see https://stackoverflow.com/a/29026565/4345715
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn get_description(schedule: &Schedule) -> String {
    let mut description = Vec::<&str>::new();

    let performer_names = schedule
        .performers
        .iter()
        .map(|p| {
            if p.tags.is_empty() {
                p.name.clone()
            } else {
                format!("{} ({})", p.name, p.tags.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    description.push(&performer_names);

    let pricing = if !schedule.currency.is_empty() && !schedule.pricing.is_empty() {
        format!("{} {}", schedule.currency, schedule.pricing)
    } else {
        String::from("")
    };
    description.push(&pricing);

    let trimmed_text = format!("\n{}\n", schedule.event.text.trim());
    description.push(&trimmed_text);

    // Google Calendar ignores URL property, add it to text
    description.push(&schedule.url);

    description.into_iter().filter(|e| !e.trim().is_empty()).collect::<Vec<_>>().join("\n")
}
