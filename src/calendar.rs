use crate::goout_api;
use icalendar::Calendar;
use reqwest::Client;
use rocket::{get, http::ContentType, request::Form, response::Content, FromForm};
use std::error::Error;

pub(in crate) type HandlerResult<T> = Result<T, Box<dyn Error>>;

#[derive(FromForm)]
pub(in crate) struct CalendarRequest {
    pub id: u64,
    pub language: String,
    pub after: Option<String>,
    pub split: bool,
}

#[get("/services/feeder/usercalendar.ics?<cal_req..>")]
pub(in crate) fn serve(cal_req: Form<CalendarRequest>) -> HandlerResult<Content<String>> {
    let client = Client::new();

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and simplicity. Disadvantage is
    // high latency of first byte served.
    let mut calendar = Calendar::new();
    for page in 1.. {
        let events_response = goout_api::fetch_page(&client, &cal_req, page)?;
        for event in goout_api::generate_events(&events_response, &cal_req)? {
            calendar.push(event);
        }

        if !events_response.has_next {
            break;
        }
    }

    Ok(Content(ContentType::Calendar, calendar.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocket;
    use mockito::mock;
    use pretty_assertions::assert_eq;
    use rocket::{http::Status, local::Client};
    use std::fs;

    #[test]
    fn test_serve_nonsplit_implicit() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_nonsplit_explicit() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&split=false",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_split() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&split=true",
            "test_data/expected_split.ical",
        );
    }

    fn invoke_serve(path: &str, expected_ical_file: &str) {
        let client = Client::new(rocket()).unwrap();

        let goout_api_path = "/services/feeder/v1/events.json?tag=liked&user=43224&page=1&language=en&source=goout.strohel.eu";
        let goout_api_mock = mock("GET", goout_api_path)
            .with_body_from_file("test_data/events.json")
            .create();

        let mut response = client.get(path).dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(ContentType::Calendar));

        let body = response.body_string().unwrap();
        let expected_body = fs::read_to_string(expected_ical_file).unwrap();
        assert_eq!(body, expected_body);

        goout_api_mock.assert();
    }
}
