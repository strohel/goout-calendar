use reqwest::{Client, Response};
use rocket::get;
use serde_json::{map::Map, value::Value};
use std::error::Error;

const ENDPOINT_URL: &str = "https://goout.net/services/feeder/v1/events.json";

#[get("/services/feeder/usercalendar.ics?<id>")]
pub(in crate) fn serve(id: u64) -> Result<String, Box<dyn Error>> {
    let client = Client::new();
    let params = &[
        ("source", "strohel.eu"),
        ("user", &id.to_string()), // TODO: seems not to actually select user?
    ];

    let mut response = client
        .get(ENDPOINT_URL)
        .query(params)
        .send()?
        .error_for_status()?;
    eprintln!("Retrieved {}.", response.url());

    let mut results: Vec<String> = Vec::new();
    let json = goout_response_json(&mut response)?;

    if let Some(schedule) = json.get("schedule") {
        results.push(parse_schedule_json(schedule)?);
    }

    Ok(results.join("\n"))
}

fn goout_response_json(response: &mut Response) -> Result<Map<String, Value>, Box<dyn Error>> {
    let json: Map<_, _> = response.json()?;
    let status = json.get("status").ok_or("No status in response.")?;
    let message = json.get("message").ok_or("No message in response.")?;
    if status != 200 {
        return Err(format!("Expected status 200, got {}.", status).into());
    }
    if message != "OK" {
        return Err(format!("Expected message OK, got {}.", message).into());
    }
    Ok(json)
}

fn parse_schedule_json(schedule: &Value) -> Result<String, Box<dyn Error>> {
    let vec = schedule
        .as_array()
        .ok_or("Value schedule is not an array.")?;

    let mut urls: Vec<&str> = Vec::new();
    let message = format!("Got {} schedule items:", vec.len());
    urls.push(&message);

    for one_schedule in vec {
        let value = one_schedule.get("url").ok_or("No url in one schedule")?;
        urls.push(value.as_str().ok_or("url is not string")?);
    }
    urls.push("");
    Ok(urls.join("\n"))
}
