use super::{DateTime, Schedule};
use crate::calendar::{CalendarRequest, LongtermHandling};
use bitflags::bitflags;
use chrono::{naive::MIN_DATE, Duration, NaiveDate, TimeZone, Utc};
use icalendar::{Component, Event as IcalEvent};
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

pub(super) fn generate_events(
    schedules: Vec<Schedule>,
    cal_req: &CalendarRequest,
) -> Vec<IcalEvent> {
    let language: &str = &cal_req.language;

    match cal_req.longterm {
        LongtermHandling::Preserve => generate_events_preserve(schedules, language),
        LongtermHandling::Split => generate_events_split(schedules, language),
        LongtermHandling::Aggregate => generate_events_aggregate(schedules, language),
    }
}

enum EventPhase {
    Begin,
    BeginEnd,
    End,
    Continued,
}

impl EventPhase {
    fn prefix(self, lang: &str) -> &'static str {
        // TODO: poor man's localization
        match (self, lang) {
            (EventPhase::Begin, "cs") => "Začátek: ",
            (EventPhase::Begin, _) => "Begin: ",
            (EventPhase::BeginEnd, "cs") => "Začátek a konec: ",
            (EventPhase::BeginEnd, _) => "Begin and end: ",
            (EventPhase::End, "cs") => "Konec: ",
            (EventPhase::End, _) => "End: ",
            (EventPhase::Continued, "cs") => "Pokračující: ",
            (EventPhase::Continued, _) => "Continued: ",
        }
    }
}

fn generate_events_preserve(schedules: Vec<Schedule>, language: &str) -> Vec<IcalEvent> {
    schedules.iter().map(|s| create_ical_event(s, language)).collect()
}

fn generate_events_split(schedules: Vec<Schedule>, language: &str) -> Vec<IcalEvent> {
    let mut events: Vec<IcalEvent> = Vec::new();
    for schedule in schedules {
        if schedule.is_long_term {
            let mut first_day_schedule = schedule.clone();
            first_day_schedule.id = 1_000_000_000_000 + schedule.id;
            Rc::make_mut(&mut first_day_schedule.event).name =
                format!("{}{}", EventPhase::Begin.prefix(language), schedule.event.name);
            first_day_schedule.end = schedule.start.date().and_hms(0, 0, 0) + Duration::days(1);
            events.push(create_ical_event(&first_day_schedule, language));

            let mut last_day_schedule = schedule.clone();
            last_day_schedule.id = 2_000_000_000_000 + schedule.id;
            Rc::make_mut(&mut last_day_schedule.event).name =
                format!("{}{}", EventPhase::End.prefix(language), schedule.event.name);
            last_day_schedule.start = schedule.end.date().and_hms(0, 0, 0) - Duration::days(1);
            events.push(create_ical_event(&last_day_schedule, language));
        } else {
            events.push(create_ical_event(&schedule, language));
        }
    }
    events
}

#[derive(Debug, Default)]
struct BreakDay<'a> {
    starts: Vec<&'a Schedule>,
    end_ids: HashSet<u64>, // exclusive, if schedule ends on day 23, we include it in day 24
}

type BreakDayMap<'a> = BTreeMap<NaiveDate, BreakDay<'a>>;

fn generate_events_aggregate(schedules: Vec<Schedule>, lang: &str) -> Vec<IcalEvent> {
    // TODO: possibly made more incremental by using forceSortByStart=ASC and tweaking algorithm.
    let mut events: Vec<IcalEvent> = Vec::new();
    let mut breakdays: BreakDayMap = BTreeMap::new();

    for schedule in schedules.iter() {
        if !schedule.is_long_term {
            events.push(create_ical_event(schedule, lang));
            continue;
        }

        let start_breakday = breakdays.entry(schedule.start.naive_local().date()).or_default();
        start_breakday.starts.push(schedule);

        let end_breakday = breakdays.entry(schedule.end.naive_local().date()).or_default();
        end_breakday.end_ids.insert(schedule.id);
    }

    render_events_from_breakdays(&mut events, &breakdays, lang);
    events
}

fn render_events_from_breakdays(events: &mut Vec<IcalEvent>, breakdays: &BreakDayMap, lang: &str) {
    let mut date_cursor: NaiveDate = MIN_DATE;
    let mut active_schedules: Vec<&Schedule> = Vec::new();

    for (&date, breakday) in breakdays.iter() {
        if !active_schedules.is_empty() {
            // TODO: mark somehow starting or ending events?
            events.push(render_aggregate_event(date_cursor, date, &active_schedules, lang));
        }
        active_schedules.retain(|s| !breakday.end_ids.contains(&s.id));
        active_schedules.extend_from_slice(&breakday.starts);
        date_cursor = date;
    }
    assert_eq!(active_schedules.len(), 0, "Active schedules not exhausted.")
}

fn render_aggregate_event(
    start: NaiveDate,
    end: NaiveDate,
    schedules: &Vec<&Schedule>,
    lang: &str,
) -> IcalEvent {
    assert_ne!(schedules.len(), 0, "render_aggregate_event() must have at least 1 schedule.");
    let mut ical_event = IcalEvent::new();

    ical_event.uid(&format!("LongTermSchedule{}@goout.net", start));

    // icalendar currently needs Date (with timezone), but doesn't actually use the TZ. Convert.
    ical_event.start_date(Utc.from_utc_date(&start));
    ical_event.end_date(Utc.from_utc_date(&end));

    if schedules.len() == 1 {
        let schedule = schedules[0];
        fill_basic_ical_event_props(&mut ical_event, schedule, lang);

        ical_event.description(&format!(
            "{} - {}\n\n{}",
            schedule.start.naive_local().date(),
            schedule.end.naive_local().date(),
            get_description(schedule, OptionalDescFields::default())
        ));
    } else {
        set_dtstamp(&mut ical_event, schedules[0]);
        ical_event.summary(&format!("{} long-term events", schedules.len())); // TODO: localisation
        ical_event.description(
            &schedules
                .iter()
                .map(|s| get_longterm_part_description(s, lang))
                .collect::<Vec<_>>()
                .join("\n\n"),
        );
    };

    ical_event
}

fn get_longterm_part_description(schedule: &Schedule, lang: &str) -> String {
    format!(
        "{}\n{} - {}\n{}",
        get_summary(schedule, lang),
        schedule.start.naive_local().date(), // TODO: deduplicate
        schedule.end.naive_local().date(),
        get_description(schedule, OptionalDescFields::empty())
    )
}

fn fill_basic_ical_event_props(ical_event: &mut IcalEvent, schedule: &Schedule, language: &str) {
    set_dtstamp(ical_event, schedule);
    ical_event.add_property("URL", &schedule.url);
    set_cancelled(ical_event, schedule.cancelled);

    let venue = &schedule.venue;
    ical_event.location(&format!(
        "{}, {}, {}, {}",
        venue.name, venue.address, venue.city, venue.locality.country.name
    ));
    ical_event.add_property("GEO", &format!("{};{}", venue.latitude, venue.longitude));

    ical_event.summary(&get_summary(schedule, language));
}

fn create_ical_event(schedule: &Schedule, language: &str) -> IcalEvent {
    let mut ical_event = IcalEvent::new();
    fill_basic_ical_event_props(&mut ical_event, schedule, language);

    ical_event.uid(&format!("Schedule#{}@goout.net", schedule.id));
    set_start_end(&mut ical_event, schedule.hour_ignored, schedule.start, schedule.end);
    ical_event.description(&get_description(schedule, OptionalDescFields::default()));

    ical_event
}

fn set_dtstamp(ical_event: &mut IcalEvent, schedule: &Schedule) {
    let uploaded_on_str = &schedule.uploaded_on.format("%Y%m%dT%H%M%S").to_string();
    ical_event.add_property("DTSTAMP", uploaded_on_str);
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

bitflags! {
    struct OptionalDescFields: u32 {
        const EVENT_TEXT = 0b00000001;
    }
}

impl Default for OptionalDescFields {
    fn default() -> Self {
        Self::EVENT_TEXT
    }
}

fn get_description(schedule: &Schedule, optional_fields: OptionalDescFields) -> String {
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

    let trimmed_text;
    if optional_fields.contains(OptionalDescFields::EVENT_TEXT) {
        trimmed_text = format!("\n{}\n", schedule.event.text.trim());
        description.push(&trimmed_text);
    }

    // Google Calendar ignores URL property, add it to text
    description.push(&schedule.url);

    description.into_iter().filter(|e| !e.trim().is_empty()).collect::<Vec<_>>().join("\n")
}
