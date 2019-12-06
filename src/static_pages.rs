use rocket::{get, response::NamedFile};
use std::io;

#[get("/")]
pub(in crate) fn index() -> io::Result<NamedFile> {
    // failure to read the file is an error, thus we want to return Result,
    // which in failure translates to HTTP 500. If we returned Option, failures
    // would result in HTTP 404, which would be misleading.
    NamedFile::open("resources/index.html")
}

#[get("/script.js")]
pub(in crate) fn script() -> io::Result<NamedFile> {
    NamedFile::open("resources/script.js")
}

#[cfg(test)]
mod tests {
    use crate::rocket;
    use rocket::http::{ContentType, Status};
    use rocket::local::Client;

    #[test]
    fn test_index() {
        test_static_page("/", ContentType::HTML, "<!DOCTYPE html>\n<html>");
    }

    #[test]
    fn test_script() {
        test_static_page(
            "/script.js?this=is&ignored",
            ContentType::JavaScript,
            "function inputChanged(",
        );
    }

    fn test_static_page(path: &str, expected_type: ContentType, expected_start: &str) {
        let client = Client::new(rocket()).unwrap();
        let mut response = client.get(path).dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.content_type(), Some(expected_type));
        assert_eq!(&response.body_string().unwrap()[..expected_start.len()], expected_start);
    }
}
