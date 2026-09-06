use axum::http::{Request, Uri};
use tower::Service;
use tower_http::services::ServeDir;

#[derive(Debug, Clone)]
pub struct SpaServeDirService<Fallback> {
    serve_dir: ServeDir<Fallback>,
}

impl<F> SpaServeDirService<F> {
    pub fn new(serve_dir: ServeDir<F>) -> Self {
        Self { serve_dir }
    }
}

impl<ReqBody, Fallback> Service<Request<ReqBody>> for SpaServeDirService<Fallback>
where
    ServeDir<Fallback>: Service<Request<ReqBody>>,
{
    type Response = <ServeDir<Fallback> as Service<Request<ReqBody>>>::Response;
    type Error = <ServeDir<Fallback> as Service<Request<ReqBody>>>::Error;
    type Future = <ServeDir<Fallback> as Service<Request<ReqBody>>>::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        <ServeDir<Fallback> as Service<Request<ReqBody>>>::poll_ready(&mut self.serve_dir, cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        if let Some(rewritten) = rewrite_uri(req.uri()) {
            *req.uri_mut() = rewritten;
        }
        self.serve_dir.call(req)
    }
}

fn rewrite_uri(in_uri: &Uri) -> Option<Uri> {
    if in_uri.path().starts_with("/albums") {
        let mut b = Uri::builder();
        if let Some(scheme) = in_uri.scheme() {
            b = b.scheme(scheme.clone());
        }
        if let Some(authority) = in_uri.authority() {
            b = b.authority(authority.clone());
        }
        b = b.path_and_query("/albums.html");
        Some(b.build().expect("url must be valid"))
    } else {
        None
    }
}

#[test]
fn url_rewriting_works() {
    assert_eq!(None, rewrite_uri(&Uri::from_static("https://example.org")));
    assert_eq!(None, rewrite_uri(&Uri::from_static("https://example.org/")));
    assert_eq!(
        None,
        rewrite_uri(&Uri::from_static("http://localhost:5173/map"))
    );
    assert_eq!(
        Some(Uri::from_static("http://localhost/albums.html")),
        rewrite_uri(&Uri::from_static("http://localhost/albums/1"))
    );
    assert_eq!(
        Some(Uri::from_static("http://localhost:5173/albums.html")),
        rewrite_uri(&Uri::from_static("http://localhost:5173/albums"))
    );
    assert_eq!(
        Some(Uri::from_static("http://subdomain.example.org/albums.html")),
        rewrite_uri(&Uri::from_static("http://subdomain.example.org/albums"))
    );
    assert_eq!(
        None,
        rewrite_uri(&Uri::from_static("https://example.org/timeline/1234"))
    );
}
