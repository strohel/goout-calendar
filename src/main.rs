#![feature(decl_macro, proc_macro_hygiene)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
#![allow(clippy::non_ascii_literal)]

use rocket::{routes, Rocket};

mod calendar;
mod error;
mod generation;
mod static_pages;

fn main() {
    rocket().launch();
}

fn rocket() -> Rocket {
    rocket::ignite().mount(
        "/",
        routes![static_pages::index, static_pages::script, calendar::serve],
    )
}
