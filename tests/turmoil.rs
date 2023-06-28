use std::{convert::Infallible, net::Ipv4Addr, sync::Arc, time::Duration};

use axum::ServiceExt;
use hyper::{server::accept::from_stream, Body, Request, Response, Uri};
use tracing::{info_span, Instrument, Span};
use turmoil_example::{connection::connector, get_server, setup_tracing};

#[test]
fn my_test() -> turmoil::Result<()> {
    let address = (Ipv4Addr::UNSPECIFIED, 9999);

    let tcp_capacity = 1000;
    let mut sim = turmoil::Builder::new().tcp_capacity(tcp_capacity).build();


    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        setup_tracing().unwrap();
    });

    let parent_span = Arc::new(info_span!("top-level-test-span"));

    let task_test_span = Arc::new(tracing::info_span!(parent: parent_span.as_ref(), "full_test"));

    let (mut tx, mut rx) = tokio::sync::oneshot::channel();
    let test_span_task_join_handle = rt.spawn(async move {
        task_test_span.enter();
        while let Err(e) = rx.try_recv() {
            tokio::time::sleep(Duration::from_millis(100)).await; 
        }
    });

    sim.host("server", move || {
        // let parent_span = &parent_span;
        async move {
            // let parent_span = parent_span.clone();
            let parent_span: Option<Arc<tracing::span::Span>> = None;
            let server_span = match &parent_span {
                Some(span) => tracing::info_span!(parent: span.as_ref(), "test_server"),
                None => tracing::info_span!("test_server"),
            };

            let router = get_server().await?;

            // Here, we use turmoil's TCP-like APIs to setup an in-memory, no-IO TcpListner
            let listener = turmoil::net::TcpListener::bind(address).await.unwrap();

            // Here, you need to use `from_stream` to convert the TcpListener into something we can use with
            // the hyper HTTP server.
            let accept = from_stream(async_stream::stream! {
                loop {
                    yield listener.accept().await.map(|(s, _)| s);
                }
            });

            tracing::log::info!("Starting test in-memory test server at {:?}", address);

            // Here, we have to use a special handler in order to connect the service up to the `accept` stream
            // formed from our custom simulation TcpListener.
            hyper::Server::builder(accept)
                .serve(hyper::service::make_service_fn(move |_| {
                    let service = router.clone();
                    async move { Ok::<_, Infallible>(service) }.instrument(Span::current())
                }))
                .await
                .unwrap();

            Ok(())
        }
        .instrument({ tracing::info_span!("server_instrument") })
    });

    let parent_span_clone = Some(parent_span.clone());
    let parent_span_clone2 = Some(parent_span.clone());
    sim.client(
        "client",
        async move {
            let client_span = match &parent_span_clone {
                Some(span) => tracing::info_span!(parent: span.as_ref(), "test_client"),
                None => tracing::info_span!("test_client"),
            };
            let client = hyper::Client::builder().build(connector());

            for i in 0..100 {
                let mut request = Request::new(Body::empty());
                *request.uri_mut() = Uri::try_from(&format!("http://server:9999/echo/{}", i)).unwrap();

                let res = client.request(request).await?;

                let (parts, body) = res.into_parts();
                let body = hyper::body::to_bytes(body).await?;
                let res = Response::from_parts(parts, body);

                tracing::info!("Got response: {:?}", res);
            }

            let mut request = Request::new(Body::empty());
            *request.uri_mut() = Uri::from_static("http://server:9999/");

            let res = client.request(request).await?;

            let (parts, body) = res.into_parts();
            let body = hyper::body::to_bytes(body).await?;
            let res = Response::from_parts(parts, body);

            tracing::info!("Got response: {:?}", res);

            Ok(())
        }
        .instrument(match &parent_span_clone2 {
            Some(span) => info_span!(parent: span.as_ref(), "client_instrument"),
            None => info_span!("client_instrument"),
        }),
    );

    drop(parent_span);

    sim.run()?;

    rt.block_on(async move {
        tx.send(()).unwrap();
        test_span_task_join_handle.await.unwrap();
    });

    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}
