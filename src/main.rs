use chrono::{NaiveDateTime, Timelike};
use reqwest::{Client};
use serde::Deserialize;

/*
use serde::de::Error;
fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let format = "%Y-%m-%d %H:%M"; // Adjust the format to match your datetime strings
    let naive_dt = NaiveDateTime::parse_from_str(&s, format)
        .map_err(D::Error::custom)?;
    Ok(Utc.from_utc_datetime(&naive_dt))
}*/

#[derive(Debug, Deserialize)]
struct HourlyResponse {
    //#[serde(deserialize_with = "deserialize_datetime")]
    //time: Vec<DateTime<Utc>>,
    time: Vec<String>,
    #[serde(alias = "temperature_2m")]
    temperature: Vec<Option<f64>>, 
    precipitation: Vec<Option<f64>>,
    snowfall: Vec<Option<f64>>
}

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    // no need for the other stuff
    hourly: HourlyResponse
}

async fn check_winter_tires(lat: f64, lon: f64, name: &str) -> Result<(), reqwest::Error> {
    let url = format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,precipitation,snowfall&forecast_days=16", lat, lon);
    let resp: WeatherResponse = Client::new().get(&url).send().await?.json().await?;
    let hours: Vec<bool> = resp.hourly
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, timestamp)| {
            let date = NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%dT%H:%M").expect("date parsing");
            let is_night = date.hour() < 8 || date.hour() > 21;
            if is_night { return None; }
            let temp_celsius = (*resp.hourly.temperature.get(i)?)?;
            let rain = (*resp.hourly.precipitation.get(i)?)?;
            Some(if rain == 0.0 {
                temp_celsius > 5.0
            } else if rain <= 0.5 {
                temp_celsius > 7.0
            } else {
                temp_celsius > 13.0
            })
        })
        .collect();

    if hours.len() < 200 {
        eprintln!("{}: too small of a sample: {} hours found", name, hours.len());
        return Ok(());
    }

    let snowfall = resp.hourly.snowfall.iter().filter_map(|x| *x).any(|x| x > 0.0);
    let summer_hours = hours.iter().filter(|x| **x).collect::<Vec<_>>().len();
    let ratio = summer_hours as f64 / hours.len() as f64;
    println!(
        "{}: {}, ratio: {}, snowfall: {}",
        name,
        if ratio > 0.6 && !snowfall { "☀️ TIME FOR SUMMER TIRES ☀️" } else { "❄️ stay on winters ❄️" },
        ratio,
        snowfall
    );
    Ok(())
}

async fn check_winter_tires_serres() -> Result<(), reqwest::Error> {
    // Serres Racing Circuit coordinates: 41.071944, 23.514722
    check_winter_tires(41.071944, 23.514722, "Serres Racing Circuit").await
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    // Original location: 42.5682, 23.1795
    let (result1, result2) = tokio::join!(
        check_winter_tires(42.5682, 23.1795, "Original Location"),
        check_winter_tires_serres()
    );
    result1?;
    result2?;
    Ok(())
}
