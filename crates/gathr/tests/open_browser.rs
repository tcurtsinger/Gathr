//! Opens a real tab in the person's default browser, so it runs only by hand:
//! `cargo test -p gathr --test open_browser -- --ignored`. The tab can be closed after.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// Shaped like Codex's ChatGPT sign-in page (1281): long, with `&`, `%` and `+` in its query.
/// Explorer opened the Documents folder for it instead of the browser.
fn sign_in_path() -> String {
    let pkce = "Zm9vYmFyYmF6cXV4cXV1eGNvcmdlZ3JhdWx0Z2FycGx5d2FsZG8";
    format!(
        "/oauth/authorize?response_type=code&client_id=app_EMoamEEZ73f0CkXaXp7hrann\
         &redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback\
         &scope=openid+profile+email+offline_access+api.connectors.read+api.connectors.invoke\
         &code_challenge={pkce}&code_challenge_method=S256&id_token_add_organizations=true\
         &codex_cli_simplified_flow=true&state={pkce}{pkce}&originator=codex_cli_rs\
         &allowed_workspace_id=&prompt=login&ui_locales=en-US%2Cen%3Bq%3D0.9"
    )
}

#[test]
#[ignore = "opens a tab in the default browser"]
fn a_long_sign_in_link_reaches_the_browser_whole() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let path = sign_in_path();
    let url = format!("http://{}{path}", listener.local_addr().unwrap());
    assert!(url.len() > 400, "the link is {} characters", url.len());

    gathr::platform::open_browser(&url).unwrap();

    listener.set_nonblocking(true).unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    let mut stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(_) if Instant::now() < until => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => panic!("no browser asked for the page in 20 s: the link didn't reach it"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut head = [0; 4096];
    let n = stream.read(&mut head).unwrap();
    let asked = String::from_utf8_lossy(&head[..n]);
    let page = "Gathr's link check passed. You can close this tab.";
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}",
        page.len()
    );
    let line = asked.lines().next().unwrap_or_default();
    assert_eq!(
        line,
        format!("GET {path} HTTP/1.1"),
        "the browser asked for another page"
    );
}
