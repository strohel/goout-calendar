#![feature(proc_macro_hygiene, decl_macro)]

use rocket::routes;

mod static_pages;

fn main() {
    rocket::ignite().mount(
        "/", routes![static_pages::index, static_pages::script]
    ).launch();
}
