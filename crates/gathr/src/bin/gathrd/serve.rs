//! The HTTP server: what `axum::serve` does, with hyper's half-closed connections on (1209). Off,
//! hyper watches each request's socket for the client hanging up; on Windows, with the service's
//! runtime threads short of CPU, that watch flooded the I/O driver and a request sat for up to
//! 30 s. axum's `serve` can't turn it on. HTTP/1 only: every client talks to 127.0.0.1 in it.

use std::future::Future;
use std::io::ErrorKind;
use std::time::Duration;

use axum::Router;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use tokio::net::TcpListener;
use tokio::sync::watch;

/// How long a stop waits for the requests under way. A connection can hold one for good (a request
/// half sent, a stream nobody ends), and the engines close only after it (G162).
const FINISH_WAIT: Duration = Duration::from_secs(5);

/// Serves `router` until `stop` ends, then lets the requests under way finish, for `FINISH_WAIT`:
/// how many connections were still open then, if any.
pub async fn serve(listener: TcpListener, router: Router, stop: impl Future<Output = ()>) -> usize {
    // Each connection holds a receiver, so `closed` returns once the last one has ended.
    let (open, connections) = watch::channel(());
    let (closing, _) = watch::channel(false);
    let mut stop = std::pin::pin!(stop);
    loop {
        let stream = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => stream,
                Err(e) => {
                    // As axum: a connection that failed before it was taken is skipped, and
                    // anything else (no handles left, say) waits a moment rather than spinning.
                    let lost = matches!(
                        e.kind(),
                        ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset | ErrorKind::ConnectionRefused
                    );
                    if !lost {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    continue;
                }
            },
            () = &mut stop => break,
        };
        let service = TowerToHyperService::new(router.clone());
        let (mut closing, held) = (closing.subscribe(), connections.clone());
        tokio::spawn(async move {
            let connection = hyper::server::conn::http1::Builder::new()
                .half_close(true)
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades();
            let mut connection = std::pin::pin!(connection);
            tokio::select! {
                _ = connection.as_mut() => {}
                () = async { _ = closing.wait_for(|c| *c).await } => {
                    connection.as_mut().graceful_shutdown();
                    let _ = connection.await;
                }
            }
            drop(held);
        });
    }
    drop(listener);
    closing.send_replace(true);
    drop(connections);
    match tokio::time::timeout(FINISH_WAIT, open.closed()).await {
        Ok(()) => 0,
        Err(_) => open.receiver_count(),
    }
}
