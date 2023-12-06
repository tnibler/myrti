use utoipa::OpenApi;

use mediathingyrust::openapi;

fn main() {
    println!("{}", openapi::ApiDoc::openapi().to_pretty_json().unwrap());
}
