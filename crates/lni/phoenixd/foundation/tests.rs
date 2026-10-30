use super::*;
use crate::cln::foundation::tests::{account, bolt12, hash, observation, preimage};
use serde_json::json;
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Step {
    method: &'static str,
    path: String,
    body: Option<Value>,
    drop_reply: bool,
    delay_ms: u64,
    expected_form: Option<BTreeMap<String, String>>,
}
fn step(method: &'static str, path: &str, body: Value) -> Step {
    Step {
        method,
        path: path.into(),
        body: Some(body),
        drop_reply: false,
        delay_ms: 0,
        expected_form: None,
    }
}
fn info() -> Value {
    json!({"nodeId":hex::encode(account().node_id),"chain":"regtest"})
}
fn handshake() -> Vec<Step> {
    vec![
        step("GET", "/getinfo", info()),
        step(
            "GET",
            "/eltor/capabilities",
            json!({"version":1,"fetchBeforePay":1,"boundedFeeMsat":1}),
        ),
    ]
}
fn expected() -> InvoiceExpectation {
    InvoiceExpectation {
        amount_msat: 2000,
        expected_payee: None,
        expected_payment_hash: None,
    }
}
fn operation() -> [u8; 16] {
    [7; 16]
}
fn request() -> DispatchRequest {
    DispatchRequest {
        operation: operation(),
        fee_limit_msat: 1000,
        retry_for_seconds: 1,
        pay_before_ms: observation().now_ms + 10000,
    }
}
fn row(invoice: &str) -> Value {
    json!({"operation":operation_text(operation()),"state":"paid","paymentHash":hex::encode(hash()),"amountMsat":2000,"feeMsat":1000,"createdAt":observation().now_ms-10,"completedAt":observation().now_ms-1,"preimage":hex::encode(preimage()),"invoice":invoice})
}
fn restore(wallet: &PhoenixdFoundation, invoice: &str) -> ValidatedInvoice {
    wallet
        .restore_for_lookup(
            invoice,
            Sha256::digest(invoice.as_bytes()).into(),
            InvoiceExpectation {
                expected_payment_hash: Some(hash()),
                ..expected()
            },
        )
        .unwrap()
}
async fn server(steps: Vec<Step>) -> (PhoenixdConfig, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        for expected in steps {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(4), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut b = [0u8; 2048];
                let n = socket.read(&mut b).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&b[..n]);
                assert!(bytes.len() < 16384);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
            let start = headers.lines().next().unwrap();
            assert_eq!(
                start,
                format!("{} {} HTTP/1.1", expected.method, expected.path)
            );
            assert!(headers.to_lowercase().contains("authorization: basic "));
            let length = headers
                .lines()
                .find_map(|s| {
                    s.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() < end + length {
                let mut b = [0u8; 2048];
                let n = socket.read(&mut b).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&b[..n]);
            }
            if let Some(form) = expected.expected_form {
                let got: BTreeMap<String, String> =
                    serde_urlencoded::from_bytes(&bytes[end..end + length]).unwrap();
                assert_eq!(got, form);
            }
            if expected.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(expected.delay_ms)).await;
            }
            if expected.drop_reply {
                continue;
            }
            let (status, body) = match expected.body {
                Some(v) => ("200 OK", serde_json::to_vec(&v).unwrap()),
                None => ("204 No Content", vec![]),
            };
            let head=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
        }
    });
    (
        PhoenixdConfig {
            url: format!("http://{address}/"),
            password: "public-fixture-password".into(),
            http_timeout: Some(3),
            ..Default::default()
        },
        worker,
    )
}
async fn finish(worker: tokio::task::JoinHandle<()>) {
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn signed_fetch_lost_dispatch_and_exact_native_recovery_never_use_payoffer() {
    let (offer, invoice) = bolt12([7; 32]);
    let mut steps = handshake();
    steps.push(step("GET", "/getinfo", info()));
    let mut fetch = step("POST", "/eltor/fetchinvoice", json!({"invoice":invoice}));
    fetch.expected_form = Some(BTreeMap::from([
        ("offer".into(), offer.clone()),
        ("amountMsat".into(), "2000".into()),
        ("payerNote".into(), hex::encode([7; 32])),
    ]));
    steps.push(fetch);
    steps.push(step("GET", "/getinfo", info()));
    let path = format!("/eltor/outgoing/{}", operation_text(operation()));
    let mut absent = step("GET", &path, json!({}));
    absent.body = None;
    steps.push(absent);
    let mut pay = step("POST", "/eltor/payinvoice", json!({}));
    pay.drop_reply = true;
    pay.expected_form = Some(BTreeMap::from([
        ("operation".into(), operation_text(operation())),
        ("invoice".into(), invoice.clone()),
        ("amountMsat".into(), "2000".into()),
        ("maxFeeMsat".into(), "1000".into()),
    ]));
    steps.push(pay);
    for _ in 0..2 {
        steps.push(step("GET", "/getinfo", info()));
        steps.push(step("GET", &path, row(&invoice)));
    }
    let (config, worker) = server(steps).await;
    let wallet = PhoenixdFoundation::connect(config, account())
        .await
        .unwrap();
    let checked = wallet
        .fetch_bolt12(&offer, [7; 32], expected(), observation())
        .await
        .unwrap();
    assert!(matches!(
        wallet.dispatch(&checked, request(), observation()).await,
        Err(AdapterError::Uncertain)
    ));
    let recovered = wallet
        .lookup(&checked, operation(), observation())
        .await
        .unwrap();
    let PaymentStatus::Settled(s) = recovered else {
        panic!("settlement missing")
    };
    assert_eq!(s.fee_msat, 1000);
    assert_eq!(s.payment_hash, hash());
    let proof = wallet
        .lookup_construction_proof(&checked, operation(), observation())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proof.preimage(), &preimage());
    finish(worker).await;
}

#[tokio::test]
async fn contradictory_native_hash_amount_operation_or_preimage_cannot_settle() {
    let (_, invoice) = bolt12([7; 32]);
    for field in [
        "paymentHash",
        "amountMsat",
        "operation",
        "preimage",
        "completedAt",
    ] {
        let mut bad = row(&invoice);
        bad[field] = match field {
            "amountMsat" => json!(2001),
            "completedAt" => json!(observation().now_ms + 100000),
            "operation" => json!(operation_text([8; 16])),
            _ => json!(hex::encode([0; 32])),
        };
        let mut steps = handshake();
        steps.push(step("GET", "/getinfo", info()));
        steps.push(step(
            "GET",
            &format!("/eltor/outgoing/{}", operation_text(operation())),
            bad,
        ));
        let (config, worker) = server(steps).await;
        let wallet = PhoenixdFoundation::connect(config, account())
            .await
            .unwrap();
        let checked = restore(&wallet, &invoice);
        assert!(wallet
            .lookup(&checked, operation(), observation())
            .await
            .is_err());
        finish(worker).await;
    }
}

#[tokio::test]
async fn preflight_latency_cannot_restart_the_dispatch_window() {
    let (_, invoice) = bolt12([7; 32]);
    let mut steps = handshake();
    let mut slow = step("GET", "/getinfo", info());
    slow.delay_ms = 1100;
    steps.push(slow);
    let mut absent = step(
        "GET",
        &format!("/eltor/outgoing/{}", operation_text(operation())),
        json!({}),
    );
    absent.body = None;
    steps.push(absent);
    let (config, worker) = server(steps).await;
    let wallet = PhoenixdFoundation::connect(config, account())
        .await
        .unwrap();
    let checked = restore(&wallet, &invoice);
    let mut request = request();
    request.pay_before_ms = observation().now_ms + 1500;
    assert!(matches!(
        wallet.dispatch(&checked, request, observation()).await,
        Err(AdapterError::InvalidInput)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn missing_profile_or_unsafe_transport_cannot_enable_payer() {
    let steps = vec![
        step("GET", "/getinfo", info()),
        step(
            "GET",
            "/eltor/capabilities",
            json!({"version":1,"fetchBeforePay":1}),
        ),
    ];
    let (config, worker) = server(steps).await;
    assert!(matches!(
        PhoenixdFoundation::connect(config, account()).await,
        Err(AdapterError::Unsupported)
    ));
    finish(worker).await;
    for url in [
        "http://wallet.example/",
        "http://100.64.0.1/",
        "http://127.0.0.1/?credential=value",
    ] {
        let config = PhoenixdConfig {
            url: url.into(),
            password: "public-fixture-password".into(),
            ..Default::default()
        };
        assert!(matches!(
            PhoenixdFoundation::connect(config, account()).await,
            Err(AdapterError::InvalidInput)
        ));
    }
}
