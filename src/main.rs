use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

async fn hello(
    mut request: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let headers = request.headers_mut();

    let Some(proxy_target) = headers.remove("Proxy-Host") else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from("Proxy-Host header is missing")))
            .unwrap());
    };

    let Ok(proxy_target) = proxy_target.to_str() else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from(
                "Proxy-Host header is not a valid string",
            )))
            .unwrap());
    };

    let Ok(mut proxy_target_uri) = proxy_target.parse::<Uri>() else {
        return Ok(Response::builder()
            .status(400)
            .body(Full::new(Bytes::from(
                "Proxy-Host header is not a valid URL",
            )))
            .unwrap());
    };

    let uri = request.uri();
    let path = uri.path();
    let raw_query = uri.query();

    let mut request_uri =
        String::with_capacity(proxy_target.len() + path.len() + raw_query.unwrap_or("").len());

    request_uri.push_str(proxy_target);
    request_uri.push_str(path);

    if let Some(raw_query) = raw_query {
        request_uri.push('?');
        request_uri.push_str(raw_query);
    }

    let mut proxy_request = Request::builder()
        .method(request.method())
        .uri(request_uri)
        .body(request.body())
        .unwrap();

    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    // We create a TcpListener and bind it to the address we want to listen on
    let listener = TcpListener::bind(addr).await?;

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(hello))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
