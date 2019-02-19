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
}

impl Event {
    fn from_html(parent: &ElementRef) -> Result<Self, Box<dyn Error>> {
        // TODO: it would be better not to do this every time, but how?
        let name_in_event_sel = Selector::parse(".name").unwrap();
        let start_time_sel = Selector::parse(".timestamp [itemprop=\"startDate\"]").unwrap();
        let end_time_sel = Selector::parse(".timestamp [itemprop=\"endDate\"]").unwrap();

        let name_elem = select_one_elem(parent, &name_in_event_sel)?;
        let name = name_elem.text().collect::<Vec<_>>().join("");

        let start_time_elem = select_one_elem(parent, &start_time_sel)?;
        let start_time = start_time_elem
            .value()
            .attr("datetime")
            .ok_or("No datetime in start_time element.")?;

        let end_time_elem = parent.select(&end_time_sel).single();
        let end_time = match end_time_elem {
            Ok(elem) => Some(
                elem.value()
                    .attr("content")
                    .ok_or("No content in end_time element.")?
                    .to_string(),
            ),
            Err(single::Error::NoElements) => None,
            Err(single::Error::MultipleElements) => {
                return Err("Multiple end_time elements.".into());
            }
        };

        Ok(Self {
            name: name.trim().to_string(),
            start_time: start_time.to_string(),
            end_time: end_time,
        })
    }
}

fn select_one_elem<'a>(
    parent_elem: &ElementRef<'a>,
    selector: &Selector,
) -> Result<ElementRef<'a>, String> {
    // map_err because following compiler message:
    // > the trait `std::error::Error` is not implemented for `single::Error`
    parent_elem.select(selector).single().map_err(|_| {
        format!(
            "Zero or multiple elements for selector {:?} in snippet {}.",
            selector,
            parent_elem.html()
        )
    })
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
        results.push(format!(
            "{}, {} -> {:?}",
            event.name, event.start_time, event.end_time
        ));
    }
    Ok(results)
}
