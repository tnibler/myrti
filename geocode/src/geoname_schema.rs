use serde::Deserialize;

/// Row in countryInfo.txt
#[derive(Debug, Deserialize)]
pub struct CountryInfoCsv {
    pub iso: String,
    pub iso3: String,
    pub iso_numeric: String,
    pub fips: String,
    pub country: String,
    pub capital: String,
    pub area: f64,
    pub population: i64,
    pub continent: String,
    pub tld: String,
    pub currency_code: String,
    pub currency_name: String,
    pub phone: String,
    pub postal_code_format: String,
    pub postal_code_regex: String,
    pub languages: String,
    pub geoname_id: i64,
    pub neighbors: String,
}

/// Row in shapes_all_low.txt
#[derive(Deserialize)]
pub struct GeoJsonCsv {
    #[serde(rename = "geoNameId")]
    pub geoname_id: i64,
    #[serde(rename = "geoJSON")]
    pub geojson: String,
}

#[derive(Deserialize)]
pub struct GeonameCsv {
    pub geoname_id: i64,
    pub name: String,
    pub asciiname: String,
    pub alternate_names: String,
    pub latitude: f32,
    pub longitude: f32,
    pub feature_class: String,
    pub feature_code: String,
    pub country_code: String,
    pub cc2: Option<String>,
    pub admin1_code: String,
    pub admin2_code: String,
    pub admin3_code: String,
    pub admin4_code: String,
    pub population: i64,
    pub elevation: Option<i32>,
    pub dem: String,
    pub timezone: String,
    pub modification_date: String,
}
