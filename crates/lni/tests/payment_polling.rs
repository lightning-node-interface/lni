use lni::{
    phoenixd::{api, PhoenixdConfig},
    ListTransactionsParams, OnInvoiceEventParams,
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

fn server(invalid_json: bool) -> PhoenixdConfig {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let polls = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            reader.read_line(&mut request).unwrap();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            let incoming = request.contains("/payments/incoming");
            let first = incoming && polls.fetch_add(1, Ordering::SeqCst) == 0;
            let (status, body) = if invalid_json {
                ("200 OK", "not JSON")
            } else if first {
                ("503 Service Unavailable", "temporary wallet outage")
            } else if incoming {
                (
                    "200 OK",
                    r#"[{"preimage":"test","paymentHash":"test-hash","receivedSat":1,"fees":0,"completedAt":1000,"createdAt":1000,"isPaid":true}]"#,
                )
            } else {
                ("200 OK", "[]")
            };
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    PhoenixdConfig {
        url,
        password: "test-only".into(),
        http_timeout: Some(2),
        ..Default::default()
    }
}

#[tokio::test]
async fn malformed_transaction_json_returns_error_without_panicking() {
    assert!(
        api::list_transactions(server(true), ListTransactionsParams { from: 0, limit: 10, payment_hash: None, search: None })
            .await
            .is_err()
    );
}

#[tokio::test]
async fn transient_poll_error_recovers_without_terminal_failure() {
    let statuses = Arc::new(Mutex::new(Vec::new()));
    let received = statuses.clone();
    api::poll_invoice_events(
        server(false),
        OnInvoiceEventParams {
            search: Some("test-hash".into()),
            polling_delay_sec: 1,
            max_polling_sec: 4,
            ..Default::default()
        },
        move |status, _| received.lock().unwrap().push(status),
    )
    .await;
    assert_eq!(*statuses.lock().unwrap(), vec!["pending", "success"]);
}
