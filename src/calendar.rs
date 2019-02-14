use html5ever::tree_builder::QuirksMode;
use reqwest::{Client, Response};
use rocket::get;
use scraper::{ElementRef, Html, Selector};
use serde_json::{map::Map, value::Value};
use single::Single;
use std::error::Error;

// This is a legacy endpoint that returns HTML snippets, which are silly to
// parse. There is /services/feeder/v1/events.json with nice JSON output, BUT
// I didn't manage to make it to display events for a particular user
// (maybe even a different endpoint should be used).
const ENDPOINT_URL: &str = "https://goout.net/legacy/follow/calendarFor";

struct Event {
    name: String,
    start_time: String,
}

impl Event {
    fn from_html(elem: &ElementRef) -> Result<Self, Box<dyn Error>> {
        // TODO: it would be better not to do this every time, but how?
        let name_in_event_sel = Selector::parse(".name").unwrap();
        let start_time_sel = Selector::parse(".timestamp [itemprop=\"startDate\"]").unwrap();

        // map_err because following compiler message:
        // > the trait `std::error::Error` is not implemented for `single::Error`
        let name_elem = elem
            .select(&name_in_event_sel)
            .single()
            .map_err(|_| "Zero or multiple name elements.")?;
        let name = name_elem.text().collect::<Vec<_>>().join("");

        let start_time_elem = elem
            .select(&start_time_sel)
            .single()
            .map_err(|_| "Zero or multiple name elements.")?;
        let start_time = start_time_elem
            .value()
            .attr("datetime")
            .ok_or("No datetime in start_time element.")?;

        Ok(Self {
            name: name.trim().to_string(),
            start_time: start_time.to_string(),
        })
    }
}

#[get("/services/feeder/usercalendar.ics?<id>")]
pub(in crate) fn serve(id: u64) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let params = &[
        ("userId", id.to_string()),
        ("future", "false".to_string()),
        ("page", 1.to_string()),
    ];

    let mut response = client
        .get(ENDPOINT_URL)
        .query(params)
        .send()?
        .error_for_status()?;
    eprintln!("Retrieved {}.", response.url());

    let json = goout_response_json(&mut response)?;
    let html_val = json.get("html").ok_or("No html key in response.")?;
    let html_str = html_val.as_str().ok_or("Key html is not a string.")?;

    parse_events_html(html_str)
}

fn goout_response_json(response: &mut Response) -> Result<Map<String, Value>, Box<dyn Error>> {
    let json: Map<_, _> = response.json()?;
    let status = json.get("status").ok_or("No status in response.")?;
    let message = json.get("message").ok_or("No message in response.")?;
    if status != 200 {
        return Err(format!("Expected status 200, got {}.", status).into());
    }
    if message != "OK" {
        return Err(format!("Expected message OK, got {}.", message).into());
    }
    Ok(json)
}

fn parse_events_html(html: &str) -> Result<String, Box<dyn Error>> {
    // See calendarForReplyExample.html file of what we need to parse.

    // unwrap() because it would be programmer error for this not to parse.
    let event_in_fragment_sel = Selector::parse(".eventCard .info").unwrap();

    let fragment = Html::parse_fragment(html);
    if fragment.errors.len() != 0 {
        return Err(fragment.errors.join("\n").into());
    }
    if fragment.quirks_mode != QuirksMode::NoQuirks {
        return Err(format!("HTML parsed only with quirks {:?}.", fragment.quirks_mode).into());
    }

    let mut results = Vec::new();
    for elem in fragment.select(&event_in_fragment_sel) {
        let event = Event::from_html(&elem)?;
        results.push(format!("{}, {} -> TODO", event.name, event.start_time));
    }
    Ok(results.join("\n"))
}
