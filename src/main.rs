use actix_web::{server, App, HttpRequest};

fn index(_req: &HttpRequest) -> &'static str {
    "Hello world!"
}

fn main() {
    let bind_addr = "127.0.0.1:8088";

    let app = server::new(|| App::new().resource("/", |r| r.f(index)))
        .bind(bind_addr)
        .unwrap();

    println!("Going to listen on {}...", bind_addr);
    app.run();
    println!("Terminating.")
}
