#![feature(decl_macro, proc_macro_hygiene)]

use rocket::routes;

mod calendar;
mod goout_api;
mod static_pages;

fn main() {
    rocket::ignite()
        .mount(
            "/",
            routes![static_pages::index, static_pages::script, calendar::serve],
        )
        .launch();
}
