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
}

#[derive(Debug, Deserialize)]
struct WeatherResponse {
    // no need for the other stuff
    hourly: HourlyResponse
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    // @TODO: flexible lat/lon
    let url = "https://api.open-meteo.com/v1/forecast?latitude=42.5682&longitude=23.1795&hourly=temperature_2m,precipitation&forecast_days=16";
    let resp: WeatherResponse = Client::new().get(url).send().await?.json().await?;
    let hours: Vec<bool> = resp.hourly
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, timestamp)| {
            let date = NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%dT%H:%M").expect("date parsing");
            let is_night = date.hour() < 8 || date.hour() > 21;
            if is_night { return None; }
            let temp_celsius = if let Some(Some(temp)) = resp.hourly.temperature.get(i) { *temp } else { return None; };
            let rain = if let Some(Some(rain)) = resp.hourly.precipitation.get(i) { *rain } else { return None; };
            if rain == 0.0 {
                Some(temp_celsius > 5.0)
            } else if rain <= 0.5 {
                Some(temp_celsius > 7.0)
            } else {
                Some(temp_celsius > 13.0)
            }
        })
        .collect();

    let summer_hours = hours.iter().filter(|x| **x).collect::<Vec<_>>().len();
    let ratio = summer_hours as f64 / hours.len() as f64;
    println!(
        "{}, ratio: {}",
        if ratio > 0.6 { "☀️ TIME FOR SUMMER TIRES ☀️" } else { "❄️ stay on winters ❄️" },
        ratio
    );
    Ok(())
}
