use crate::{error::HandlerError, generation};
use rocket::{
    get,
    http::{ContentType, RawStr, Status},
    request::{FormParseError, LenientForm},
    response::Content,
    FromForm, FromFormValue,
};

#[derive(Debug, FromFormValue)]
pub(in crate) enum LongtermHandling {
    /// Preserve long-term events as-is: have multiple of them per day.
    Preserve,
    /// Split start and end events for long-term events.
    Split,
    /// Aggregate long-term events to max one per day.
    Aggregate,
}

pub(in crate) struct CalendarRequest {
    pub id: u64,
    pub language: String,
    pub after: Option<String>,
    pub longterm: LongtermHandling,
}

// Compatibility struct to accept both v1 (split: bool) and v2 (longterm: LongtermHandling) of the API
#[derive(Debug, FromForm)]
pub(in crate) struct CompatibleCalendarRequest<'a> {
    id: u64,
    language: String,
    after: Option<String>,
    // Option needs to be there to tell false from not present; Result needs to be there to tell parse error from not present:
    split: Option<Result<bool, &'a RawStr>>,
    longterm: Option<Result<LongtermHandling, &'a RawStr>>,
}

#[get("/services/feeder/usercalendar.ics?<compat_cal_req_form..>")]
pub(in crate) fn serve(
    compat_cal_req_form: Result<LenientForm<CompatibleCalendarRequest>, FormParseError>,
) -> Result<Content<String>, HandlerError> {
    let compat_cal_req = compat_cal_req_form?.into_inner();
    // For Err variants, we mimic internal Rocket behaviour: return the same parse error
    let longterm = match (compat_cal_req.split, compat_cal_req.longterm) {
        (None, None) => LongtermHandling::Preserve, // default
        (None, Some(Ok(longterm_match))) => longterm_match,
        (None, Some(Err(err))) => {
            return Err(FormParseError::BadValue("longterm".into(), err).into())
        }
        (Some(Ok(true)), None) => LongtermHandling::Split,
        (Some(Ok(false)), None) => LongtermHandling::Preserve,
        (Some(Err(err)), None) => return Err(FormParseError::BadValue("split".into(), err).into()),
        (Some(_), Some(_)) => return Err(HandlerError::new(
            Status::BadRequest,
            "Bad request: Please drop the deprecated 'split' parameter when using 'longterm'.\n"
                .to_string(),
        )),
    };
    let cal_req = CalendarRequest {
        id: compat_cal_req.id,
        language: compat_cal_req.language,
        after: compat_cal_req.after,
        longterm,
    };

    // Normally, we would stream to output as soon as we get first page, but
    // instead we load all pages first and only then start replying. We can
    // afford this, because the calendar endpoint would be typically called
    // infrequently and in non-interactive manner. Advantage is that we can
    // properly report errors on HTTP level, and simplicity. Disadvantage is
    // high latency of first byte served.
    let calendar_string = generation::generate(&cal_req)?;
    Ok(Content(ContentType::Calendar, calendar_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocket;
    use mockito::mock;
    use pretty_assertions::assert_eq;
    use rocket::local::Client;
    use std::fs;

    #[test]
    fn test_serve() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_longterm_preserve() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&longterm=preserve",
            "test_data/expected_nonsplit.ical",
        );
    }

    #[test]
    fn test_serve_longterm_split() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&longterm=split",
            "test_data/expected_split.ical",
        );
    }

    #[test]
    fn test_serve_longterm_aggregate() {
        invoke_serve(
            "/services/feeder/usercalendar.ics?id=43224&language=en&longterm=aggregate",
            "test_data/expected_aggregate.ical",
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

    #[test]
    fn test_invalid_serve_no_id() {
        invoke_serve_lowlevel(
            "/services/feeder/usercalendar.ics?language=en",
            Status::BadRequest,
            "text/plain; charset=utf-8",
            "Bad request: Missing(RawStr(\"id\")) (see https://api.rocket.rs/v0.4/rocket/request/enum.FormParseError.html)\n"
        );
    }

    #[test]
    fn test_invalid_serve_no_language() {
        invoke_serve_lowlevel(
            "/services/feeder/usercalendar.ics?id=123",
            Status::BadRequest,
            "text/plain; charset=utf-8",
            "Bad request: Missing(RawStr(\"language\")) (see https://api.rocket.rs/v0.4/rocket/request/enum.FormParseError.html)\n"
        );
    }

    #[test]
    fn test_invalid_serve_bad_id() {
        invoke_serve_lowlevel(
            "/services/feeder/usercalendar.ics?id=nckcd&language=cs",
            Status::BadRequest,
            "text/plain; charset=utf-8",
            "Bad request: BadValue(RawStr(\"id\"), RawStr(\"nckcd\")) (see https://api.rocket.rs/v0.4/rocket/request/enum.FormParseError.html)\n"
        );
    }

    #[test]
    fn test_invalid_serve_bad_split() {
        invoke_serve_lowlevel(
            "/services/feeder/usercalendar.ics?id=123&language=cs&split=gogo",
            Status::BadRequest,
            "text/plain; charset=utf-8",
            "Bad request: BadValue(RawStr(\"split\"), RawStr(\"gogo\")) (see https://api.rocket.rs/v0.4/rocket/request/enum.FormParseError.html)\n"
        );
    }

    #[test]
    fn test_invalid_serve_both_split_longterm() {
        invoke_serve_lowlevel(
            "/services/feeder/usercalendar.ics?id=123&language=cs&split=true&longterm=split",
            Status::BadRequest,
            "text/plain; charset=utf-8",
            "Bad request: Please drop the deprecated 'split' parameter when using 'longterm'.\n",
        );
    }

    #[test]
    fn test_invalid_serve_bad_longterm() {
        invoke_serve_lowlevel(
            "/services/feeder/usercalendar.ics?id=123&language=cs&longterm=gagagogo",
            Status::BadRequest,
            "text/plain; charset=utf-8",
            "Bad request: BadValue(RawStr(\"longterm\"), RawStr(\"gagagogo\")) (see https://api.rocket.rs/v0.4/rocket/request/enum.FormParseError.html)\n"
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
        let goout_api_path = format!("/services/feeder/v1/events.json?{}", goout_api_params);
        let goout_api_mock = mock("GET", goout_api_path.as_str())
            .with_body_from_file(format!("test_data/{}", goout_api_resp_file))
            .create();

        let expected_body = fs::read_to_string(expected_ical_file).unwrap();
        invoke_serve_lowlevel(path, Status::Ok, "text/calendar", &expected_body);

        goout_api_mock.assert();
    }

    fn invoke_serve_lowlevel(
        path: &str,
        expected_status: Status,
        expected_content_type: &str,
        expected_body: &str,
    ) {
        let client = Client::new(rocket()).unwrap();
        let mut response = client.get(path).dispatch();

        let content_type = response.content_type().unwrap().to_string();
        let body = response.body_string().unwrap();
        // compare all at once for most descriptive failure messages by pretty_assertions
        assert_eq!(
            (response.status(), content_type.as_ref(), body.as_ref()),
            (expected_status, expected_content_type, expected_body)
        );
    }
}
