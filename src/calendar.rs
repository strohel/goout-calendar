use html5ever::tree_builder::QuirksMode;
use reqwest::{Client, Response};
use rocket::get;
use scraper::{ElementRef, Html, Selector};
use serde_json::{map::Map, value::Value};
use single::{self, Single};
use std::error::Error;

// This is a legacy endpoint that returns HTML snippets, which are silly to
// parse. There is /services/feeder/v1/events.json with nice JSON output, BUT
// I didn't manage to make it to display events for a particular user
// (maybe even a different endpoint should be used).
const ENDPOINT_URL: &str = "https://goout.net/legacy/follow/calendarFor";

struct Event {
    name: String,
    start_time: String,
    end_time: Option<String>,
    venue: String,
    street_address: Option<String>,
    address_locality: String,
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
    fn from_html(parent: &ElementRef) -> Result<Self, Box<dyn Error>> {
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
            select_one_optional(selector_str)?.ok_or_else(|| format!(
                "No elements matching {} in {}.",
                selector_str,
                parent.html()
            ))
        };

        use event_selectors::*;
        let name = Self::extract_text(&select_one(NAME)?);
        let start_time = Self::extract_attr(&select_one(START_TIME)?, "datetime")?;
        let end_time = select_one_optional(END_TIME)?
            .map(|ref e| Self::extract_attr(e, "content"))
            .transpose()?;
        let venue = Self::extract_text(&select_one(VENUE)?);
        let street_address = select_one_optional(STREET_ADDRESS)?
            .map(|ref e| Self::extract_attr(e, "content"))
            .transpose()?;
        let address_locality = Self::extract_attr(&select_one(ADDRESS_LOCALITY)?, "content")?;

        Ok(Self {
            name,
            start_time,
            end_time,
            venue,
            street_address,
            address_locality,
        })
    }

    fn extract_text(element: &ElementRef) -> String {
        element
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string()
    }

    fn extract_attr(element: &ElementRef, name: &str) -> Result<String, Box<dyn Error>> {
        let value = element
            .value()
            .attr(name)
            .ok_or_else(|| format!("No attribute {} in element.", name))?;
        Ok(value.to_string())
    }
}

#[get("/services/feeder/usercalendar.ics?<id>")]
pub(in crate) fn serve(id: u64) -> Result<String, Box<dyn Error>> {
    let client = Client::new();

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and siplicity. Disadvantage is
    // high latency of first byte served.
    let mut lines = Vec::new();
    for page in 1.. {
        let json = fetch_page(&client, id, page)?;
        let (html_str, has_next) = parse_json_reply(&json)?;
        lines.extend(parse_events_html(html_str)?);

        if !has_next {
            break;
        }
    }

    Ok(lines.join("\n"))
}

fn fetch_page(client: &Client, id: u64, page: u16) -> Result<Map<String, Value>, Box<dyn Error>> {
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

fn parse_json_reply(json: &Map<String, Value>) -> Result<(&str, bool), Box<dyn Error>> {
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

fn parse_events_html(html: &str) -> Result<Vec<String>, Box<dyn Error>> {
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

    let mut results = Vec::new();
    for elem in fragment.select(&event_in_fragment_sel) {
        let e = Event::from_html(&elem)?;
        results.push(format!(
            "{} at {} ({:?}, {})  {} -> {:?}",
            e.name, e.venue, e.street_address, e.address_locality, e.start_time, e.end_time
        ));
    }
    Ok(results)
}
