use chrono::{NaiveDate, NaiveDateTime, Timelike};
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
    time: Vec<String>,
    #[serde(alias = "temperature_2m")]
    temperature: Vec<Option<f64>>, 
    precipitation: Vec<Option<f64>>,
    snowfall: Vec<Option<f64>>
}

#[derive(Debug, Deserialize)]
struct DailySeasonalResponse {
    time: Vec<String>,
    #[serde(alias = "temperature_2m_max")]
    temperature_max: Vec<Option<f64>>,
    #[serde(alias = "temperature_2m_min")]
    temperature_min: Vec<Option<f64>>,
    #[serde(alias = "precipitation_sum")]
    precipitation_sum: Vec<Option<f64>>,
    #[serde(alias = "rain_sum")]
    rain_sum: Vec<Option<f64>>,
    #[serde(alias = "snowfall_sum")]
    snowfall_sum: Vec<Option<f64>>,
    #[serde(alias = "snowfall_water_equivalent_sum")]
    snowfall_water_equivalent_sum: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum WeatherData {
    Hourly {
        hourly: HourlyResponse
    },
    Daily {
        daily: DailySeasonalResponse
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiResponse {
    Error {
        #[serde(rename = "error")]
        error: bool,
        reason: String,
    },
    Success(WeatherData),
}


async fn check_winter_tires(lat: f64, lon: f64, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,precipitation,snowfall&forecast_days=16    ", lat, lon);
    let api_resp: ApiResponse = Client::new().get(&url).send().await?.json().await?;
    
    let hourly = match api_resp {
        ApiResponse::Error { reason, .. } => {
            return Err(format!("API error for {}: {}", name, reason).into());
        }
        ApiResponse::Success(WeatherData::Hourly { hourly }) => hourly,
        ApiResponse::Success(WeatherData::Daily { .. }) => {
            return Err(format!("Expected hourly data for {}, got daily data", name).into());
        }
    };
    
    let hours: Vec<bool> = hourly
        .time
        .iter()
        .enumerate()
        .filter_map(|(i, timestamp)| {
            let date = NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%dT%H:%M").expect("date parsing");
            let is_night = date.hour() < 8 || date.hour() > 21;
            if is_night { return None; }
            let temp_celsius = (*hourly.temperature.get(i)?)?;
            let rain = (*hourly.precipitation.get(i)?)?;
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

    let snowfall = hourly.snowfall.iter().filter_map(|x| *x).any(|x| x > 0.0);
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

async fn check_trackday_windows() -> Result<(), Box<dyn std::error::Error>> {
    // Serres Racing Circuit coordinates: 41.071944, 23.514722
    // Get 70 days of daily forecast from seasonal API
    let url = format!("https://seasonal-api.open-meteo.com/v1/seasonal?latitude=41.071944&longitude=23.514722&daily=temperature_2m_max,temperature_2m_min,precipitation_sum,rain_sum,snowfall_sum,snowfall_water_equivalent_sum&forecast_days=70&timezone=auto");
    let api_resp: ApiResponse = Client::new().get(&url).send().await?.json().await?;
    
    let daily = match api_resp {
        ApiResponse::Error { reason, .. } => {
            return Err(format!("API error for Serres Racing Circuit: {}", reason).into());
        }
        ApiResponse::Success(WeatherData::Daily { daily }) => daily,
        ApiResponse::Success(WeatherData::Hourly { .. }) => {
            return Err("Expected daily data for Serres Racing Circuit, got hourly data".into());
        }
    };
    
    let mut trackday_dates = Vec::new();
    let len = daily.time.len();
    
    // Check each day (starting from index 3 to have 3 days before available)
    for i in 3..(len.saturating_sub(2)) {
        let min_temp = match daily.temperature_min.get(i) {
            Some(Some(t)) => *t,
            _ => continue,
        };
        
        let max_temp = match daily.temperature_max.get(i) {
            Some(Some(t)) => *t,
            _ => continue,
        };
        
        // Check temperature conditions
        if min_temp <= 8.0 || max_temp <= 15.0 {
            continue;
        }
        
        // Check rain conditions: past 3 days (i-3, i-2, i-1), current day (i), and next 2 days (i+1, i+2)
        let mut has_rain = false;
        let start = i.saturating_sub(3);
        let end = (i + 2).min(len - 1);
        for j in start..=end {
            let precip = daily.precipitation_sum.get(j)
                .and_then(|x| *x)
                .unwrap_or(0.0);
            if precip > 0.0 {
                has_rain = true;
                break;
            }
        }
        
        if !has_rain {
            let date = NaiveDate::parse_from_str(&daily.time[i], "%Y-%m-%d")
                .expect("date parsing");
            trackday_dates.push((date, min_temp, max_temp));
        }
    }
    
    println!("Serres Racing Circuit - Trackday Windows (next 70 days):");
    if trackday_dates.is_empty() {
        println!("  No suitable trackday windows found.");
    } else {
        for (date, min_temp, max_temp) in trackday_dates {
            println!("  {} - Min: {:.1}°C, Max: {:.1}°C", date, min_temp, max_temp);
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Original location: 42.5682, 23.1795
    check_winter_tires(42.5682, 23.1795, "Original Location").await?;
    check_trackday_windows().await?;
    Ok(())
}
