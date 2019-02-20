#![feature(decl_macro, proc_macro_hygiene, transpose_result)]

use rocket::routes;

mod calendar;
mod static_pages;

fn main() {
    rocket::ignite()
        .mount(
            "/",
            routes![static_pages::index, static_pages::script, calendar::serve],
        )
        .launch();
}
