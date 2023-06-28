use std::{future::Future, pin::Pin};

use hyper::{
    client::connect::{Connected, Connection},
    Uri,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tower::Service;
use turmoil::net::TcpStream;

type Fut = Pin<Box<dyn Future<Output = Result<TurmoilConnection, std::io::Error>> + Send>>;

pub fn connector(
) -> impl Service<Uri, Response = TurmoilConnection, Error = std::io::Error, Future = Fut> + Clone {
    tower::service_fn(|uri: Uri| {
        Box::pin(async move {
            let conn = TcpStream::connect(uri.authority().unwrap().as_str()).await?;
            Ok::<_, std::io::Error>(TurmoilConnection(conn))
        }) as Fut
    })
}

pub struct TurmoilConnection(turmoil::net::TcpStream);

impl AsyncRead for TurmoilConnection {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for TurmoilConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl Connection for TurmoilConnection {
    fn connected(&self) -> hyper::client::connect::Connected {
        Connected::new()
    }
}
