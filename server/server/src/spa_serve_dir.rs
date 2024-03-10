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
        if req.uri().path().starts_with("/albums") {
            let mut b = Uri::builder();
            if let Some(scheme) = req.uri().scheme() {
                b = b.scheme(scheme.clone());
            }
            if let Some(authority) = req.uri().authority() {
                b = b.authority(authority.clone());
            }
            b = b.path_and_query("albums.html");
            *req.uri_mut() = b.build().expect("url is copied from request");
        }
        self.serve_dir.call(req)
    }
}
