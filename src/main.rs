use hyper::server::conn::http1;
use hyper::{Request, Response, Uri, body::Incoming, StatusCode};
use hyper_util::rt::TokioIo;
use reqwest::Client;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::cors::CorsLayer;
use http_body_util::{BodyExt, Full};
use http_body_util::combinators::BoxBody;
use bytes::Bytes;
use tower::service_fn;
use std::convert::Infallible;
use hyper_util::service::TowerToHyperService;
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::env;

// The remote host to proxy requests to.
fn remote_host() -> String {
    env::var("REMOTE_HOST").unwrap_or_else(|_| "127.0.0.1:8000".to_string())
}

async fn proxy(
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let span = info_span!("proxy", method = %req.method(), uri = %req.uri());
    let _enter = span.enter();

    let client = Client::new();

    // Create a new URI for the downstream request.
    let path = req.uri().path_and_query().map(|x| x.as_str()).unwrap_or("");
    let uri_string = format!("http://{}{}", remote_host(), path);

    let new_uri = match uri_string.parse::<Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            tracing::error!(error = %e, "Invalid URI");
            return Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Full::new(Bytes::from("Invalid URI")))
                .unwrap())
        }
    };

    // Convert the hyper::Request into a reqwest::Request.
    let mut new_req_builder = client.request(req.method().clone(), new_uri.to_string());

    for (key, value) in req.headers() {
        new_req_builder = new_req_builder.header(key, value);
    }

    let body_bytes = match req.into_body().collect().await {
        Ok(body) => body.to_bytes(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to read request body");
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Failed to read request body")))
                .unwrap())
        }
    };

    let new_req = match new_req_builder.body(body_bytes).build() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build request");
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Failed to build request")))
                .unwrap())
        }
    };

    // Send the request using reqwest.
    let res = match client.execute(new_req).await {
        Ok(res) => res,
        Err(e) => {
            tracing::error!(error = %e, "Failed to execute request");
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Failed to execute request")))
                .unwrap())
        }
    };

    // Convert the reqwest::Response back into a hyper::Response.
    let mut builder = Response::builder().status(res.status());
    for (key, value) in res.headers() {
        builder = builder.header(key, value);
    }

    let body = match res.bytes().await {
        Ok(body) => body,
        Err(e) => {
            tracing::error!(error = %e, "Failed to read response body");
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from("Failed to read response body")))
                .unwrap())
        }
    };

    let response = builder.body(Full::new(body)).unwrap();

    Ok(response)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "vibes_reverse_proxy=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await.unwrap();

    let proxy_service = service_fn(proxy)
        .map_response(|res| res.map(|b| b.boxed()));

    // Create a service that is cloneable.
    let service = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|_req: &Request<Incoming>| {
                    tracing::debug_span!("request")
                })
                .on_response(|res: &Response<BoxBody<Bytes, Infallible>>, latency, _span: &Span| {
                    tracing::debug!(status = %res.status(), ?latency, "response");
                })
        )
        .layer(CorsLayer::permissive())
        .service(proxy_service);

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let service_clone = service.clone();

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, TowerToHyperService::new(service_clone))
                .await
            {
                tracing::error!(error = %err, "server error");
            }
        });
    }
}
