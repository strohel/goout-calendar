#![feature(proc_macro_hygiene, decl_macro)]

use rocket::routes;

mod index;

fn main() {
    rocket::ignite().mount("/", routes![index::serve]).launch();
}
