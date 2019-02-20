use crate::calendar::{Event, HandlerResult};
use html5ever::tree_builder::QuirksMode;
use reqwest::{Client, Response};
use scraper::{ElementRef, Html, Selector};
use serde_json::{map::Map, value::Value};
use single::{self, Single};

// This is a legacy endpoint that returns HTML snippets, which are silly to
// parse. There is /services/feeder/v1/events.json with nice JSON output, BUT
// I didn't manage to make it to display events for a particular user
// (maybe even a different endpoint should be used).
const ENDPOINT_URL: &str = "https://goout.net/legacy/follow/calendarFor";

pub(in crate) fn fetch_page(
    client: &Client,
    id: u64,
    page: u16,
) -> HandlerResult<Map<String, Value>> {
    let params = &[
        ("userId", id.to_string()),
        ("future", "false".to_string()),
        ("page", page.to_string()),
    ];

    let mut response = client
        .get(ENDPOINT_URL)
        .query(params)
        .send()?
        .error_for_status()?;
    eprintln!("Retrieved {}.", response.url());

    goout_response_json(&mut response)
}

fn goout_response_json(response: &mut Response) -> HandlerResult<Map<String, Value>> {
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

pub(in crate) fn parse_json_reply(json: &Map<String, Value>) -> HandlerResult<(&str, bool)> {
    let html_str = json
        .get("html")
        .ok_or("No html key in response.")?
        .as_str()
        .ok_or("Key html is not a string.")?;
    let has_next = json
        .get("hasNext")
        .ok_or("No hasNext key in response.")?
        .as_bool()
        .ok_or("Key hasNext is not a bool")?;
    Ok((html_str, has_next))
}

pub(in crate) fn parse_events_html(html: &str) -> HandlerResult<Vec<Event>> {
    // See calendarForReplyExample.html file of what we need to parse.

    // unwrap() because it would be programmer error for this not to parse.
    let event_in_fragment_sel = Selector::parse(fragment_selectors::EVENT).unwrap();

    let fragment = Html::parse_fragment(html);
    if !fragment.errors.is_empty() {
        return Err(fragment.errors.join("\n").into());
    }
    if fragment.quirks_mode != QuirksMode::NoQuirks {
        return Err(format!("HTML parsed only with quirks {:?}.", fragment.quirks_mode).into());
    }

    let mut events = Vec::new();
    for elem in fragment.select(&event_in_fragment_sel) {
        let event = Event::from_html(&elem)?;
        events.push(event);
    }
    Ok(events)
}

mod fragment_selectors {
    pub const EVENT: &str = ".eventCard .info";
}

mod event_selectors {
    pub const NAME: &str = ".name[itemprop=name]";
    pub const START_TIME: &str = "[itemprop=startDate]";
    pub const END_TIME: &str = "[itemprop=endDate]";
    // pub const DESCRIPTION: &str = "[itemprop=description]"; TODO: not in API reply
    pub const VENUE: &str = "[itemprop=location] [itemprop=name]";
    pub const STREET_ADDRESS: &str = "[itemprop=streetAddress]";
    pub const ADDRESS_LOCALITY: &str = "[itemprop=addressLocality]";
    // TODO: type of event, first text of <div class="timestamp">
}

impl Event {
    fn from_html(parent: &ElementRef) -> HandlerResult<Self> {
        let select_one_optional = |selector_str| {
            // TODO: it would be better not to do this every time, but how?
            let selector = Selector::parse(selector_str).unwrap();
            let element_result = parent.select(&selector).single();
            match element_result {
                Ok(element) => Ok(Some(element)),
                Err(single::Error::NoElements) => Ok(None),
                Err(single::Error::MultipleElements) => Err(format!(
                    "Multiple elements matching {} in {}.",
                    selector_str,
                    parent.html()
                )),
            }
        };
        let select_one = |selector_str| {
            select_one_optional(selector_str)?.ok_or_else(|| {
                format!(
                    "No elements matching {} in {}.",
                    selector_str,
                    parent.html()
                )
            })
        };

        use event_selectors::*;
        let name = extract_text(&select_one(NAME)?);
        let start_time = extract_attr(&select_one(START_TIME)?, "datetime")?;
        let end_time = select_one_optional(END_TIME)?
            .map(|ref e| extract_attr(e, "content"))
            .transpose()?;
        let venue = extract_text(&select_one(VENUE)?);
        let street_address = select_one_optional(STREET_ADDRESS)?
            .map(|ref e| extract_attr(e, "content"))
            .transpose()?;
        let address_locality = extract_attr(&select_one(ADDRESS_LOCALITY)?, "content")?;

        Ok(Self {
            name,
            start_time,
            end_time,
            venue,
            street_address,
            address_locality,
        })
    }
}

fn extract_text(element: &ElementRef) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
}

fn extract_attr(element: &ElementRef, name: &str) -> HandlerResult<String> {
    let value = element
        .value()
        .attr(name)
        .ok_or_else(|| format!("No attribute {} in element.", name))?;
    Ok(value.to_string())
}
