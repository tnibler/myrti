use std::eprintln;

use color_eyre::eyre::{eyre, Context, Result};
use geocode::{Coordinates, ReverseGeocoder};

fn parse_lat_lon(s: &str) -> Result<(f32, f32)> {
    match s.trim().split(",").map(|s| s.trim()).collect::<Vec<_>>() {
        split if split.len() == 2 => {
            let f: Vec<f32> = split
                .into_iter()
                .map(|s| s.parse().wrap_err("parse error"))
                .collect::<Result<_>>()?;
            Ok((f[0], f[1]))
        }
        _ => Err(eyre!("invalid input")),
    }
}

#[tokio::main]
async fn main() {
    eprintln!("initializing");
    let rgc = ReverseGeocoder::new("geodata.db".into()).await.unwrap();
    if rgc.base_data_present().await.unwrap().is_none() {
        eprintln!("downloading country list");
        rgc.download_base_data().await.unwrap();
    }

    eprintln!("ready");
    let mut input = String::new();
    while let Ok(_) = std::io::stdin().read_line(&mut input) {
        let (lat, lon): (f32, f32) = match parse_lat_lon(&input) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("error parsing input");
                input.clear();
                continue;
            }
        };
        let coords = Coordinates { lat, lon };
        let lookup = match rgc.lookup(coords).await {
            Ok(result) => result,
            Err(geocode::ReverseGeocodeError::CountryDataNotPresent { country_id }) => {
                eprintln!("downloading geoname data for country {}", country_id.0);
                rgc.download_country_data(country_id).await.unwrap();
                rgc.lookup(coords).await.unwrap()
            }
            Err(e) => Err(e).unwrap(),
        };
        println!("{:?}", lookup);
        input.clear();
    }
    rgc.close().await;
}
