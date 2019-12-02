use super::{DateTime, Event, Schedule};
use crate::calendar::CalendarRequest;
use chrono::{Duration, Utc};
use icalendar::{Component, Event as IcalEvent};

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

pub(super) fn generate_events(
    schedules: Vec<Schedule>,
    cal_req: &CalendarRequest,
) -> Vec<IcalEvent> {
    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in schedules {
        if schedule.is_long_term && cal_req.split {
            let first_day_end = schedule.start.date().and_hms(23, 59, 59);
            let prefix = match &cal_req.language[..] {
                "cs" => "Začátek: ",
                _ => "Begin: ",
            };
            let info = schedule_info(schedule.start, first_day_end, prefix);
            events.push(create_ical_event(&schedule, &info, cal_req));

            let last_day_start = schedule.end.date().and_hms(0, 0, 0);
            let prefix = match &cal_req.language[..] {
                "cs" => "Konec: ",
                _ => "End: ",
            };
            let info = schedule_info(last_day_start, schedule.end, prefix);
            events.push(create_ical_event(&schedule, &info, cal_req));
        } else {
            let info = schedule_info(schedule.start, schedule.end, "");
            events.push(create_ical_event(&schedule, &info, cal_req));
        }
    }
    events
}

fn create_ical_event(
    schedule: &Schedule,
    info: &ScheduleInfo,
    cal_req: &CalendarRequest,
) -> IcalEvent {
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

    let venue = &schedule.venue;
    ical_event.location(&format!(
        "{}, {}, {}, {}",
        venue.name, venue.address, venue.city, venue.locality.country.name
    ));
    ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

    set_summary(
        &mut ical_event,
        &schedule.event,
        info.summary_prefix,
        schedule.cancelled,
        cal_req,
    );
    set_description(&mut ical_event, schedule);

    #[cfg(debug_assertions)]
    {
        eprintln!("Parsed {:?} {:?} as:", schedule, info);
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
        &description
            .into_iter()
            .filter(|e| !e.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
