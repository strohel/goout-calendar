use anyhow;
use rocket::{
    http::Status,
    request::{FormParseError, Request},
    response::{status::Custom, Responder, Response},
};

pub(in crate) type HandlerResult<T> = Result<T, anyhow::Error>;

#[derive(Debug)]
pub(in crate) struct HandlerError {
    pub responder: Custom<String>,
}

impl HandlerError {
    pub const fn new(status: Status, text: String) -> Self {
        let responder = Custom(status, text);
        Self { responder }
    }
}

impl<'r> Responder<'r> for HandlerError {
    fn respond_to(self, req: &Request) -> Result<Response<'r>, Status> {
        self.responder.respond_to(req)
    }
}

impl From<FormParseError<'_>> for HandlerError {
    fn from(parse_error: FormParseError) -> Self {
        Self::new(Status::BadRequest, format!(
            "Bad request: {:?} (see https://api.rocket.rs/v0.4/rocket/request/enum.FormParseError.html)\n",
            parse_error))
    }
}

impl From<anyhow::Error> for HandlerError {
    fn from(e: anyhow::Error) -> Self {
        eprintln!("Uncaught handler error: {}", e);
        Self::new(Status::InternalServerError, "Something went wrong.".to_string())
    }
}
