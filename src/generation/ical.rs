use super::{DateTime, Event, Schedule};
use crate::calendar::{CalendarRequest, LongtermHandling};
use chrono::{Duration, Utc};
use icalendar::{Component, Event as IcalEvent};
use std::rc::Rc;

pub(super) fn generate_events(
    schedules: Vec<Schedule>,
    cal_req: &CalendarRequest,
) -> Vec<IcalEvent> {
    let language: &str = &cal_req.language;
    let begin_prefix = match language {
        "cs" => "Začátek: ",
        _ => "Begin: ",
    };
    let end_prefix = match language {
        "cs" => "Konec: ",
        _ => "End: ",
    };

    let split = match cal_req.longterm {
        LongtermHandling::Split => true,
        LongtermHandling::Preserve => false,
        LongtermHandling::Aggregate => unimplemented!(),
    };

    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in schedules {
        if schedule.is_long_term && split {
            let mut first_day_schedule = schedule.clone();
            first_day_schedule.id = 1_000_000_000_000 + schedule.id;
            Rc::make_mut(&mut first_day_schedule.event).name =
                format!("{}{}", begin_prefix, schedule.event.name);
            first_day_schedule.end = schedule.start.date().and_hms(23, 59, 59);
            events.push(create_ical_event(&first_day_schedule, language));

            let mut last_day_schedule = schedule.clone();
            last_day_schedule.id = 2_000_000_000_000 + schedule.id;
            Rc::make_mut(&mut last_day_schedule.event).name =
                format!("{}{}", end_prefix, schedule.event.name);
            last_day_schedule.start = schedule.end.date().and_hms(0, 0, 0);
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

    set_summary(&mut ical_event, &schedule.event, schedule.cancelled, language);
    set_description(&mut ical_event, schedule);

    #[cfg(debug_assertions)]
    {
        eprintln!("Parsed {:?} as:", schedule);
        eprintln!("{}", ical_event.to_string());
    }

    ical_event
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

fn set_summary(ical_event: &mut IcalEvent, event: &Event, cancelled: bool, language: &str) {
    let cancelled_prefix = if cancelled {
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

fn set_description(ical_event: &mut IcalEvent, schedule: &Schedule) {
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

    ical_event.description(
        &description.into_iter().filter(|e| !e.trim().is_empty()).collect::<Vec<_>>().join("\n"),
    );
}
