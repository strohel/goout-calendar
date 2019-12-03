use crate::{error::HandlerError, generation};
use reqwest::Client;
use rocket::{
    get,
    http::ContentType,
    request::{FormParseError, LenientForm},
    response::Content,
    FromForm,
};

#[derive(FromForm)]
pub(in crate) struct CalendarRequest {
    pub id: u64,
    pub language: String,
    pub after: Option<String>,
    pub split: bool,
}

#[get("/services/feeder/usercalendar.ics?<cal_req..>")]
pub(in crate) fn serve(
    cal_req: Result<LenientForm<CalendarRequest>, FormParseError>,
) -> Result<Content<String>, HandlerError> {
    let cal_req = cal_req?;
    let client = Client::new();

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and simplicity. Disadvantage is
    // high latency of first byte served.
    let calendar_string = generation::generate(&client, &cal_req)?;
    Ok(Content(ContentType::Calendar, calendar_string))
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
    fn test_serve_bad_request() {
        let client = Client::new(rocket()).unwrap();
        let mut response = client.get("/services/feeder/usercalendar.ics").dispatch();
        assert_eq!(response.status(), Status::BadRequest);

        let body = response.body_string().unwrap();
        let expected_start = "Bad request: Missing(RawStr(\"id\"))";
        assert_eq!(&body[..expected_start.len()], expected_start);
    }

    #[test]
    fn test_serve() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_split_false() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&split=false",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_split_true() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&split=true",
            "test_data/expected_split.ical",
        );
    }

    #[test]
    fn test_serve_after() {
        invoke_serve_ex(
            "/services/feeder/usercalendar.ics?id=43224&language=en&after=2020-04-01",
            "tag=liked&user=43224&page=1&language=en&source=goout.strohel.eu&after=2020-04-01",
            "events_empty.json",
            "test_data/expected_empty.ical",
        );
    }

    #[test]
    fn test_serve_split_true_after() {
        invoke_serve_ex(
            "/services/feeder/usercalendar.ics?id=43224&split=true&language=en&after=2020-04-01",
            "tag=liked&user=43224&page=1&language=en&source=goout.strohel.eu&after=2020-04-01",
            "events_empty.json",
            "test_data/expected_empty.ical",
        );
    }

    #[test]
    fn test_serve_extraparam() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&extraparam=value",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_split_true_extraparam() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&extraparam=value&split=true",
            "test_data/expected_split.ical",
        );
    }

    fn invoke_serve(path: &str, expected_ical_file: &str) {
        invoke_serve_ex(
            path,
            "tag=liked&user=43224&page=1&language=en&source=goout.strohel.eu",
            "events.json",
            expected_ical_file,
        )
    }

    fn invoke_serve_ex(
        path: &str,
        goout_api_params: &str,
        goout_api_resp_file: &str,
        expected_ical_file: &str,
    ) {
        let client = Client::new(rocket()).unwrap();

        let goout_api_path = format!("/services/feeder/v1/events.json?{}", goout_api_params);
        let goout_api_mock = mock("GET", goout_api_path.as_str())
            .with_body_from_file(format!("test_data/{}", goout_api_resp_file))
            .create();

        let mut response = client.get(path).dispatch();

        let content_type_string = response.content_type().unwrap().to_string();
        let body = response.body_string().unwrap();
        let expected_body = fs::read_to_string(expected_ical_file).unwrap();
        // compare all at once for most descriptive failure messages by pretty_assertions
        assert_eq!(
            (response.status(), content_type_string.as_ref(), body),
            (Status::Ok, "text/calendar", expected_body)
        );

        goout_api_mock.assert();
    }
}
