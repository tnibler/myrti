use utoipa::OpenApi;

use myrti::openapi;

fn to_camel_case(s: &str) -> String {
    let mut cs = String::new();
    cs.reserve(s.len());
    let mut it = s.chars().into_iter().peekable();
    while let Some(c) = it.next() {
        match (c, it.peek()) {
            ('_', Some(nc)) if *nc != '_' => {
                cs.push(nc.to_ascii_uppercase());
                let _ = it.next(); // consume nc
            }
            (c, _) => {
                cs.push(c);
            }
        }
    }
    cs
}

fn main() {
    let mut oapi: utoipa::openapi::OpenApi = openapi::ApiDoc::openapi();
    // convert operationIds from snake_case to camelCase
    oapi.paths.paths.iter_mut().for_each(|(_path, path_item)| {
        path_item.operations.iter_mut().for_each(|(_, op)| {
            op.operation_id = op.operation_id.as_ref().map(|name| to_camel_case(name));
        });
    });
    println!("{}", oapi.to_pretty_json().unwrap());
}
