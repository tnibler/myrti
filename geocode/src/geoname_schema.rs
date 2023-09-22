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

/// Row in main geonames country CSV file
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

/// Populated place
/// https://download.geonames.org/export/dump/featureCodes_en.txt
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PopFeatureType {
    /// populated place	a city, town, village, or other agglomeration of buildings where people live and work
    PPL,
    /// seat of a first-order administrative division	seat of a first-order administrative division (PPLC takes precedence over PPLA)
    PPLA,
    /// seat of a second-order administrative division
    PPLA2,
    /// seat of a third-order administrative division
    PPLA3,
    /// seat of a fourth-order administrative division
    PPLA4,
    /// seat of a fifth-order administrative division
    PPLA5,
    /// capital of a political entity
    PPLC,
    /// historical capital of a political entity	a former capital of a political entity
    PPLCH,
    /// farm village	a populated place where the population is largely engaged in agricultural activities
    PPLF,
    /// seat of government of a political entity
    PPLG,
    /// historical populated place	a populated place that no longer exists
    PPLH,
    /// populated locality	an area similar to a locality but with a small group of dwellings or other buildings
    PPLL,
    /// abandoned populated place
    PPLQ,
    /// religious populated place	a populated place whose population is largely engaged in religious occupations
    PPLR,
    /// populated places	cities, towns, villages, or other agglomerations of buildings where people live and work
    PPLS,
    /// destroyed populated place	a village, town or city destroyed by a natural disaster, or by war
    PPLW,
    /// section of populated place
    PPLX,
    /// israeli settlement
    STLMT,
    /// causeway
    /// a raised roadway across wet ground or shallow water
    CSWY,
    /// oil pipeline	a pipeline used for transporting oil
    OILP,
    /// promenade
    /// a place for public walking, usually along a beach front
    PRMN,
    /// portage
    /// a place where boats, goods, etc., are carried overland between navigable waters
    PTGE,
    /// road
    /// an open way with improved surface for transportation of animals, people and vehicles
    RD,
    /// ancient road	the remains of a road used by ancient cultures
    RDA,
    /// road bend	a conspicuously curved or bent section of a road
    RDB,
    /// road cut	an excavation cut through a hill or ridge for a road
    RDCUT,
    /// road junction	a place where two or more roads join
    RDJCT,
    /// railroad junction	a place where two or more railroad tracks join
    RJCT,
    /// railroad
    /// a permanent twin steel-rail track on which freight and passenger cars move long distances
    RR,
    /// abandoned railroad
    RRQ,
    /// caravan route	the route taken by caravans
    RTE,
    /// railroad yard	a system of tracks used for the making up of trains, and switching and storing freight cars
    RYD,
    /// street
    /// a paved urban thoroughfare
    ST,
    /// stock route	a route taken by livestock herds
    STKR,
    /// tunnel
    /// a subterranean passageway for transportation
    TNL,
    /// natural tunnel	a cave that is open at both ends
    TNLN,
    /// road tunnel	a tunnel through which a road passes
    TNLRD,
    /// railroad tunnel	a tunnel through which a railroad passes
    TNLRR,
    /// tunnels
    /// subterranean passageways for transportation
    TNLS,
    /// trail
    /// a path, track, or route used by pedestrians, animals, or off-road vehicles
    TRL,
}
