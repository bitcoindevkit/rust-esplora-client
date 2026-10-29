//! Local Esplora HTTP fixtures for the WASM smoke tests.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::fs;
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use std::{env, process, thread};

    use tiny_http::{Header, ListenAddr, Method, Response, Server, StatusCode};

    fn cors_headers<R: std::io::Read>(response: &mut Response<R>) {
        response.add_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap());
        response.add_header(Header::from_bytes("Access-Control-Allow-Headers", "*").unwrap());
        response.add_header(
            Header::from_bytes("Access-Control-Allow-Methods", "GET, OPTIONS").unwrap(),
        );
    }

    fn respond(request: tiny_http::Request, status: u16, body: &str) {
        let mut response = Response::from_string(body).with_status_code(StatusCode(status));
        cors_headers(&mut response);
        let _ = request.respond(response);
    }

    fn handle(request: tiny_http::Request, flaky: &AtomicUsize) {
        let method = request.method().clone();
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("");

        if method == Method::Options {
            let mut response = Response::empty(204);
            cors_headers(&mut response);
            let _ = request.respond(response);
            return;
        }

        match (method, path) {
            (Method::Get, "/blocks/tip/height") => respond(request, 200, "101"),
            (Method::Get, "/fee-estimates") => respond(request, 200, r#"{"1":1.5,"2":2.5}"#),
            (Method::Post, "/tx") => {
                let mut request = request;
                let mut body = String::new();
                let is_hex_tx = request.as_reader().read_to_string(&mut body).is_ok()
                    && !body.is_empty()
                    && body.bytes().all(|b| b.is_ascii_hexdigit());
                if is_hex_tx {
                    respond(
                        request,
                        200,
                        "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
                    );
                } else {
                    respond(request, 400, "expected a hex-encoded transaction body");
                }
            }
            (Method::Get, "/flaky/blocks/tip/height") => {
                if flaky.fetch_add(1, Ordering::SeqCst) == 0 {
                    respond(request, 503, "retry");
                } else {
                    respond(request, 200, "101");
                }
            }
            (Method::Get, "/slow/blocks/tip/height") => {
                thread::sleep(Duration::from_secs(2));
                respond(request, 200, "101");
            }
            _ => respond(request, 404, "not found"),
        }
    }

    let mut args = env::args().skip(1);
    let url_file = match (args.next().as_deref(), args.next()) {
        (Some("--url-file"), Some(path)) => path,
        _ => {
            eprintln!("usage: fixture --url-file <path>");
            process::exit(2);
        }
    };

    let server =
        Server::http(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).expect("bind fixture server");
    let addr = match server.server_addr() {
        ListenAddr::IP(addr) => addr,
        other => panic!("unexpected listen address: {other:?}"),
    };
    let base_url = format!("http://{addr}");
    fs::write(&url_file, &base_url).expect("write url file");
    eprintln!("Serving Esplora WASM fixtures at {base_url}");

    let flaky = Arc::new(AtomicUsize::new(0));
    for request in server.incoming_requests() {
        let flaky = Arc::clone(&flaky);
        thread::spawn(move || handle(request, &flaky));
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
