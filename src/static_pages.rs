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
