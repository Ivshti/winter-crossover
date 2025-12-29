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
struct SeasonalWeatherResponse {
    daily: DailySeasonalResponse
}


async fn check_winter_tires(lat: f64, lon: f64, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,precipitation,snowfall&forecast_days=16", lat, lon);
    let response = Client::new().get(&url).send().await?;
    let text = response.text().await?;
    let resp: WeatherResponse = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Unexpected response for {}:", name);
            eprintln!("{}", text);
            return Err(format!("Failed to deserialize response: {}", e).into());
        }
    };
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

async fn check_trackday_windows() -> Result<(), Box<dyn std::error::Error>> {
    // Serres Racing Circuit coordinates: 41.071944, 23.514722
    // Get 70 days of daily forecast from seasonal API
    let url = format!("https://seasonal-api.open-meteo.com/v1/seasonal?latitude=41.071944&longitude=23.514722&daily=temperature_2m_max,temperature_2m_min,precipitation_sum,rain_sum,snowfall_sum,snowfall_water_equivalent_sum&forecast_days=70&timezone=auto");
    let response = Client::new().get(&url).send().await?;
    let text = response.text().await?;
    let resp: SeasonalWeatherResponse = match serde_json::from_str(&text) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Unexpected response for Serres Racing Circuit:");
            eprintln!("{}", text);
            return Err(format!("Failed to deserialize response: {}", e).into());
        }
    };
    
    // Convert daily data to vector of (date, min_temp, max_temp, precip_sum)
    let mut days: Vec<(NaiveDate, f64, f64, f64)> = Vec::new();
    
    for (i, date_str) in resp.daily.time.iter().enumerate() {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .expect("date parsing");
        
        let min_temp = match resp.daily.temperature_min.get(i) {
            Some(Some(t)) => *t,
            _ => continue,
        };
        
        let max_temp = match resp.daily.temperature_max.get(i) {
            Some(Some(t)) => *t,
            _ => continue,
        };
        
        let precip_sum = match resp.daily.precipitation_sum.get(i) {
            Some(Some(p)) => *p,
            _ => 0.0,
        };
        
        days.push((date, min_temp, max_temp, precip_sum));
    }
    
    let mut trackday_dates = Vec::new();
    
    // Check each day (starting from index 3 to have 3 days before available)
    for i in 3..(days.len().saturating_sub(2)) {
        let (date, min_temp, max_temp, _) = days[i];
        
        // Check temperature conditions
        if min_temp <= 8.0 || max_temp <= 15.0 {
            continue;
        }
        
        // Check rain conditions: past 3 days (i-3, i-2, i-1), current day (i), and next 2 days (i+1, i+2)
        let mut has_rain = false;
        let start = i.saturating_sub(3);
        let end = (i + 2).min(days.len() - 1);
        for j in start..=end {
            let (_, _, _, precip) = days[j];
            if precip > 0.0 {
                has_rain = true;
                break;
            }
        }
        
        if !has_rain {
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
