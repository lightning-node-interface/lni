use super::*;
use lightning::bitcoin::{
    hashes::{sha256, Hash},
    secp256k1::{Keypair, Secp256k1, SecretKey},
};
use lightning_invoice::{InvoiceBuilder, PaymentSecret};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) fn observation() -> Observation {
    Observation {
        now_ms: 1_700_000_100_000,
        uncertainty_ms: 0,
    }
}
fn key(n: u8) -> SecretKey {
    SecretKey::from_slice(&[n; 32]).unwrap()
}
fn pubkey(n: u8) -> [u8; 33] {
    PublicKey::from_secret_key(&Secp256k1::new(), &key(n)).serialize()
}
pub(crate) fn account() -> NativeAccount {
    NativeAccount {
        node_id: pubkey(1),
        chain: Chain::Regtest,
    }
}
fn expectation() -> InvoiceExpectation {
    InvoiceExpectation {
        amount_msat: 2000,
        expected_payee: Some(pubkey(2)),
        expected_payment_hash: None,
    }
}
pub(crate) fn preimage() -> [u8; 32] {
    [42; 32]
}
pub(crate) fn hash() -> [u8; 32] {
    Sha256::digest(preimage()).into()
}
fn bolt11() -> String {
    bolt11_for(2)
}
fn bolt11_for(signer: u8) -> String {
    InvoiceBuilder::new(Currency::Regtest)
        .description("local fixture".to_owned())
        .payment_hash(sha256::Hash::from_byte_array(hash()))
        .payment_secret(PaymentSecret([3; 32]))
        .duration_since_epoch(Duration::from_secs(1_700_000_000))
        .expiry_time(Duration::from_secs(3600))
        .min_final_cltv_expiry_delta(18)
        .amount_milli_satoshis(2000)
        .build_signed(|m| Secp256k1::new().sign_ecdsa_recoverable(m, &key(signer)))
        .unwrap()
        .to_string()
}

pub(crate) fn bolt12(note: [u8; 32]) -> (String, String) {
    use lightning::{
        blinded_path::payment::{
            BlindedPaymentPath, Bolt12RefundContext, PaymentConstraints, PaymentContext,
            UnauthenticatedReceiveTlvs,
        },
        ln::{channelmanager::PaymentId, inbound_payment::ExpandedKey},
        offers::{invoice::UnsignedBolt12Invoice, nonce::Nonce, offer::OfferBuilder},
        sign::EntropySource,
        types::payment::PaymentHash,
        util::ser::Writeable,
    };
    struct Entropy;
    impl EntropySource for Entropy {
        fn get_secure_random_bytes(&self) -> [u8; 32] {
            [43; 32]
        }
    }
    let secp = Secp256k1::new();
    let expanded = ExpandedKey::new([44; 32]);
    let entropy = Entropy;
    let nonce = Nonce::from_entropy_source(&entropy);
    let receiver = PublicKey::from_secret_key(&secp, &key(2));
    let keys = Keypair::from_secret_key(&secp, &key(2));
    let offer = OfferBuilder::new(receiver)
        .chain(Network::Regtest)
        .amount_msats(2000)
        .build()
        .unwrap();
    let request = offer
        .request_invoice(&expanded, nonce, &secp, PaymentId([45; 32]))
        .unwrap()
        .chain(Network::Regtest)
        .unwrap()
        .payer_note(hex::encode(note))
        .build_and_sign()
        .unwrap();
    let tlvs = UnauthenticatedReceiveTlvs {
        payment_secret: PaymentSecret([46; 32]),
        payment_constraints: PaymentConstraints {
            max_cltv_expiry: u32::MAX,
            htlc_minimum_msat: 1,
        },
        payment_context: PaymentContext::Bolt12Refund(Bolt12RefundContext {}),
    }
    .authenticate(nonce, &expanded);
    let path = BlindedPaymentPath::one_hop(receiver, tlvs, 18, &entropy, &secp).unwrap();
    let invoice = request
        .respond_with_no_std(
            vec![path],
            PaymentHash(hash()),
            Duration::from_secs(1_700_000_000),
        )
        .unwrap()
        .relative_expiry(3600)
        .build()
        .unwrap()
        .sign(|m: &UnsignedBolt12Invoice| {
            Ok::<_, ()>(secp.sign_schnorr_no_aux_rand(m.as_ref().as_digest(), &keys))
        })
        .unwrap();
    let mut bytes = Vec::new();
    invoice.write(&mut bytes).unwrap();
    (
        offer.to_string(),
        bech32::encode::<NoChecksum>(Hrp::parse("lni").unwrap(), &bytes).unwrap(),
    )
}
fn request() -> DispatchRequest {
    DispatchRequest {
        operation: [3; 16],
        fee_limit_msat: 100,
        retry_for_seconds: 5,
        pay_before_ms: 1_700_000_110_000,
    }
}
fn info() -> Value {
    json!({"id":hex::encode(pubkey(1)),"network":"regtest","version":"v25.09"})
}
fn row(invoice: &str, status: &str) -> Value {
    json!({"payment_hash":hex::encode(hash()),"label":hex::encode(request().operation),"bolt11":invoice,"status":status,"created_at":1_700_000_000u64,"completed_at":1_700_000_100u64,"amount_msat":2000,"amount_sent_msat":2050,"preimage":hex::encode(preimage())})
}
struct Step {
    path: &'static str,
    body: Option<Value>,
    reply: Option<Value>,
}
fn step(path: &'static str, reply: Value) -> Step {
    Step {
        path,
        body: None,
        reply: Some(reply),
    }
}
fn handshake() -> Vec<Step> {
    vec![
        step("getinfo", info()),
        step(
            "help",
            json!({"help":[{"command":"pay bolt11 [label] [maxfee]"}]}),
        ),
        step(
            "help",
            json!({"help":[{"command":"listpays [payment_hash]"}]}),
        ),
        step(
            "help",
            json!({"help":[{"command":"fetchinvoice offer [amount_msat] [payer_note]"}]}),
        ),
    ]
}

async fn server(steps: Vec<Step>) -> (ClnConfig, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let worker = tokio::spawn(async move {
        for expected in steps {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let header_end;
            loop {
                let mut next = [0; 1024];
                let n = stream.read(&mut next).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&next[..n]);
                assert!(bytes.len() < 131072);
                if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = at + 4;
                    break;
                }
            }
            let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
            assert!(headers.starts_with(&format!("POST /v1/{} HTTP/1.1\r\n", expected.path)));
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() - header_end < length {
                let mut next = [0; 1024];
                let n = stream.read(&mut next).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&next[..n]);
            }
            let body: Value =
                serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
            if let Some(expected) = expected.body {
                assert!(body == expected);
            }
            if let Some(reply) = expected.reply {
                let body = reply.to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
        }
    });
    (
        ClnConfig {
            url: format!("http://{address}"),
            rune: "fixture-only-rune".into(),
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
async fn native_fee_limited_bolt11_dispatch_and_exact_lookup_use_msat_and_private_label() {
    let invoice = bolt11();
    let mut steps = handshake();
    steps.extend([step("getinfo",info()),step("listpays",json!({"pays":[]})),Step{path:"pay",body:Some(json!({"bolt11":invoice,"maxfee":"100msat","label":hex::encode(request().operation),"retry_for":5})),reply:Some(json!({"status":"complete"}))},step("getinfo",info()),step("listpays",json!({"pays":[row(&invoice,"complete")]}))]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let prepared = adapter
        .inspect_bolt11(&invoice, expectation(), observation())
        .unwrap();
    let result = adapter
        .dispatch(&prepared, request(), observation())
        .await
        .unwrap();
    let PaymentStatus::Settled(receipt) = result else {
        panic!("expected native settlement");
    };
    assert_eq!(receipt.amount_msat, 2000);
    assert_eq!(receipt.fee_msat, 50);
    assert_eq!(receipt.settled_latest_ms - receipt.settled_earliest_ms, 999);
    finish(worker).await;
}

#[tokio::test]
async fn native_pending_record_never_causes_another_pay() {
    let invoice = bolt11();
    let mut pending = row(&invoice, "pending");
    pending.as_object_mut().unwrap().remove("preimage");
    pending.as_object_mut().unwrap().remove("completed_at");
    let mut steps = handshake();
    steps.extend([
        step("getinfo", info()),
        step("listpays", json!({"pays":[pending]})),
    ]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let invoice = adapter
        .inspect_bolt11(&invoice, expectation(), observation())
        .unwrap();
    assert!(
        adapter
            .dispatch(&invoice, request(), observation())
            .await
            .unwrap()
            == PaymentStatus::Pending
    );
    finish(worker).await;
}

#[tokio::test]
async fn pay_transport_loss_is_unknown_and_lookup_recovers_without_redispatch() {
    let invoice = bolt11();
    let mut steps = handshake();
    steps.extend([
        step("getinfo", info()),
        step("listpays", json!({"pays":[]})),
        Step {
            path: "pay",
            body: None,
            reply: None,
        },
        step("getinfo", info()),
        step("listpays", json!({"pays":[row(&invoice,"complete")]})),
    ]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let invoice = adapter
        .inspect_bolt11(&invoice, expectation(), observation())
        .unwrap();
    assert!(matches!(
        adapter.dispatch(&invoice, request(), observation()).await,
        Err(AdapterError::Uncertain)
    ));
    assert!(matches!(
        adapter
            .lookup(&invoice, request().operation, observation())
            .await
            .unwrap(),
        PaymentStatus::Settled(_)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn identity_or_fee_capability_mismatch_fails_before_any_payment() {
    let (config, worker) = server(vec![step(
        "getinfo",
        json!({"id":hex::encode(pubkey(2)),"network":"regtest"}),
    )])
    .await;
    assert!(matches!(
        ClnFoundation::connect(config, account()).await,
        Err(AdapterError::AccountMismatch)
    ));
    finish(worker).await;
    let (config, worker) = server(vec![
        step("getinfo", info()),
        step(
            "help",
            json!({"help":[{"command":"pay bolt11 [label] [maxfeepercent]"}]}),
        ),
    ])
    .await;
    assert!(matches!(
        ClnFoundation::connect(config, account()).await,
        Err(AdapterError::Unsupported)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn invoice_must_match_native_signature_chain_payee_amount_and_expiry() {
    let (config, worker) = server(handshake()).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let invoice = bolt11();
    let mut expected = expectation();
    expected.amount_msat += 1;
    assert!(adapter
        .inspect_bolt11(&invoice, expected, observation())
        .is_err());
    expected = expectation();
    expected.expected_payee = Some(pubkey(1));
    assert!(adapter
        .inspect_bolt11(&invoice, expected, observation())
        .is_err());
    expected = expectation();
    expected.expected_payment_hash = Some([5; 32]);
    assert!(adapter
        .inspect_bolt11(&invoice, expected, observation())
        .is_err());
    assert!(adapter
        .inspect_bolt11(&invoice.to_ascii_uppercase(), expectation(), observation())
        .is_err());
    assert!(adapter
        .inspect_bolt11(
            &invoice,
            expectation(),
            Observation {
                now_ms: 1_700_004_000_000,
                uncertainty_ms: 0
            }
        )
        .is_err());
    let mut corrupted = invoice.into_bytes();
    let n = corrupted.len();
    corrupted[n - 1] = if corrupted[n - 1] == b'q' { b'p' } else { b'q' };
    assert!(adapter
        .inspect_bolt11(
            std::str::from_utf8(&corrupted).unwrap(),
            expectation(),
            observation()
        )
        .is_err());
    finish(worker).await;
}

#[tokio::test]
async fn missing_duplicate_or_wrong_label_native_records_remain_unknown() {
    let (config, worker) = server(handshake()).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let text = bolt11();
    let prepared = adapter
        .inspect_bolt11(&text, expectation(), observation())
        .unwrap();
    let valid = row(&text, "complete");
    assert!(
        adapter
            .interpret(&prepared, request().operation, &[], observation())
            .unwrap()
            == PaymentStatus::Unknown
    );
    assert!(
        adapter
            .interpret(
                &prepared,
                request().operation,
                &[valid.clone(), valid.clone()],
                observation()
            )
            .unwrap()
            == PaymentStatus::Unknown
    );
    for field in ["label", "payment_hash", "bolt11"] {
        let mut wrong = valid.clone();
        wrong[field] = json!("wrong");
        assert!(
            adapter
                .interpret(&prepared, request().operation, &[wrong], observation())
                .unwrap()
                == PaymentStatus::Unknown
        );
    }
    finish(worker).await;
}

#[tokio::test]
async fn complete_status_requires_correct_proof_amount_fees_and_native_time() {
    let (config, worker) = server(handshake()).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let text = bolt11();
    let prepared = adapter
        .inspect_bolt11(&text, expectation(), observation())
        .unwrap();
    for field in 0..5 {
        let mut wrong = row(&text, "complete");
        match field {
            0 => wrong["preimage"] = json!(hex::encode([4; 32])),
            1 => wrong["amount_msat"] = json!(2001),
            2 => wrong["amount_sent_msat"] = json!(1999),
            3 => wrong["completed_at"] = json!(1_700_000_200u64),
            _ => wrong["amount_sent_msat"] = json!(u64::MAX),
        };
        assert!(adapter
            .interpret(&prepared, request().operation, &[wrong], observation())
            .is_err());
    }
    let mut string_units = row(&text, "complete");
    string_units["amount_msat"] = json!("2000msat");
    string_units["amount_sent_msat"] = json!("2050msat");
    assert!(matches!(
        adapter
            .interpret(
                &prepared,
                request().operation,
                &[string_units],
                observation()
            )
            .unwrap(),
        PaymentStatus::Settled(_)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn bolt12_fetch_is_separate_from_pay_and_verifies_native_offer_invoice_and_note() {
    let payment_id = [7; 32];
    let (offer, invoice) = bolt12(payment_id);
    let mut steps = handshake();
    steps.extend([step("getinfo",info()),Step{path:"fetchinvoice",body:Some(json!({"offer":offer,"amount_msat":"2000msat","payer_note":hex::encode(payment_id),"timeout":30})),reply:Some(json!({"invoice":invoice}))}]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let mut expected = expectation();
    expected.expected_payee = None;
    let validated = adapter
        .fetch_bolt12(&offer, payment_id, expected, observation())
        .await
        .unwrap();
    assert_eq!(validated.amount_msat, 2000);
    assert!(validated.payment_hash == hash());
    assert_eq!(validated.invoice().len(), invoice.len());
    finish(worker).await;
}

#[tokio::test]
async fn bolt12_changed_note_or_amount_is_rejected_before_pay() {
    let (offer, invoice) = bolt12([7; 32]);
    let mut steps = handshake();
    for _ in 0..2 {
        steps.extend([
            step("getinfo", info()),
            step("fetchinvoice", json!({"invoice":invoice})),
        ]);
    }
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    assert!(adapter
        .fetch_bolt12(&offer, [8; 32], expectation(), observation())
        .await
        .is_err());
    let mut expected = expectation();
    expected.amount_msat = 2001;
    assert!(adapter
        .fetch_bolt12(&offer, [7; 32], expected, observation())
        .await
        .is_err());
    finish(worker).await;
}

#[tokio::test]
async fn bolt12_native_payment_uses_the_inspected_invoice_and_recovers_exact_record() {
    let (offer, invoice) = bolt12([7; 32]);
    let mut native = row(&invoice, "complete");
    native.as_object_mut().unwrap().remove("bolt11");
    native["bolt12"] = json!(invoice);
    let mut steps = handshake();
    steps.extend([step("getinfo",info()),step("fetchinvoice",json!({"invoice":invoice})),step("getinfo",info()),step("listpays",json!({"pays":[]})),Step{path:"pay",body:Some(json!({"bolt11":invoice,"maxfee":"100msat","label":hex::encode(request().operation),"retry_for":5})),reply:Some(json!({"status":"complete"}))},step("getinfo",info()),step("listpays",json!({"pays":[native]}))]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let validated = adapter
        .fetch_bolt12(&offer, [7; 32], expectation(), observation())
        .await
        .unwrap();
    assert!(matches!(
        adapter
            .dispatch(&validated, request(), observation())
            .await
            .unwrap(),
        PaymentStatus::Settled(_)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn terminal_failure_requires_a_single_bound_record_without_paid_parts() {
    let (config, worker) = server(handshake()).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let invoice = bolt11();
    let prepared = adapter
        .inspect_bolt11(&invoice, expectation(), observation())
        .unwrap();
    let mut failed = row(&invoice, "failed");
    failed.as_object_mut().unwrap().remove("preimage");
    failed.as_object_mut().unwrap().remove("amount_sent_msat");
    assert!(
        adapter
            .interpret(
                &prepared,
                request().operation,
                &[failed.clone()],
                observation()
            )
            .unwrap()
            == PaymentStatus::FailedFinal
    );
    failed["amount_sent_msat"] = json!(1);
    assert!(
        adapter
            .interpret(
                &prepared,
                request().operation,
                &[failed.clone()],
                observation()
            )
            .unwrap()
            == PaymentStatus::Unknown
    );
    failed.as_object_mut().unwrap().remove("amount_sent_msat");
    assert!(
        adapter
            .interpret(
                &prepared,
                request().operation,
                &[failed.clone(), failed],
                observation()
            )
            .unwrap()
            == PaymentStatus::Unknown
    );
    finish(worker).await;
}

#[tokio::test]
async fn account_change_after_connection_prevents_dispatch() {
    let mut steps = handshake();
    steps.push(step(
        "getinfo",
        json!({"id":hex::encode(pubkey(2)),"network":"regtest"}),
    ));
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let invoice = adapter
        .inspect_bolt11(&bolt11(), expectation(), observation())
        .unwrap();
    assert!(matches!(
        adapter.dispatch(&invoice, request(), observation()).await,
        Err(AdapterError::AccountMismatch)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn secret_bearing_configuration_and_nonlocal_plain_http_are_rejected_without_requests() {
    assert!(canonical_text("lni1+continuation").is_err());
    assert!(msat(Some(&json!("0.2sat"))).is_err());
    assert!(seconds(Some(&json!(u64::MAX))).is_err());
    assert!(
        account().fingerprint()
            != NativeAccount {
                node_id: pubkey(1),
                chain: Chain::Bitcoin
            }
            .fingerprint()
    );
    for url in [
        "http://wallet.example/",
        "http://fixture:fixture@127.0.0.1/",
        "https://127.0.0.1/?fixture=query",
    ] {
        let config = ClnConfig {
            url: url.into(),
            rune: "fixture-only-rune".into(),
            ..Default::default()
        };
        assert!(matches!(
            ClnFoundation::connect(config, account()).await,
            Err(AdapterError::InvalidInput)
        ));
    }
    let config = ClnConfig {
        accept_invalid_certs: Some(true),
        rune: "fixture-only-rune".into(),
        ..Default::default()
    };
    assert!(matches!(
        ClnFoundation::connect(config, account()).await,
        Err(AdapterError::InvalidInput)
    ));
}

#[tokio::test]
async fn receiving_tls_override_is_explicit_private_and_never_outgoing() {
    for url in [
        "https://127.0.0.1/",
        "https://100.64.0.1/",
        "https://100.127.255.254/",
    ] {
        assert!(receiving_test_tls_endpoint(
            &reqwest::Url::parse(url).unwrap()
        ));
        let config = ClnConfig {
            url: url.into(),
            accept_invalid_certs: Some(true),
            rune: "fixture-only-rune".into(),
            ..Default::default()
        };
        // Rejected before a socket/request, including on a private destination.
        assert!(matches!(
            ClnFoundation::connect(config, account()).await,
            Err(AdapterError::InvalidInput)
        ));
    }
    for url in [
        "http://100.64.0.1/",
        "https://100.63.255.255/",
        "https://100.128.0.1/",
        "https://192.168.1.1/",
        "https://wallet.example/",
        "https://8.8.8.8/",
    ] {
        assert!(!receiving_test_tls_endpoint(
            &reqwest::Url::parse(url).unwrap()
        ));
        let config = ClnConfig {
            url: url.into(),
            accept_invalid_certs: Some(true),
            rune: "fixture-only-rune".into(),
            ..Default::default()
        };
        assert!(matches!(
            ClnFoundation::connect_receiving(config, account()).await,
            Err(AdapterError::InvalidInput)
        ));
    }
    let config = ClnConfig {
        url: "https://100.64.0.1/".into(),
        accept_invalid_certs: Some(true),
        socks5_proxy: Some("socks5h://127.0.0.1:1".into()),
        rune: "fixture-only-rune".into(),
        ..Default::default()
    };
    assert!(matches!(
        ClnFoundation::connect_receiving(config, account()).await,
        Err(AdapterError::InvalidInput)
    ));
}

#[tokio::test]
async fn bolt12_invalid_native_signature_cannot_be_used_for_payment() {
    let (offer, invoice) = bolt12([7; 32]);
    let mut bytes = invoice.into_bytes();
    let at = bytes.len() - 12;
    bytes[at] = if bytes[at] == b'q' { b'p' } else { b'q' };
    let invalid = String::from_utf8(bytes).unwrap();
    let mut steps = handshake();
    steps.extend([
        step("getinfo", info()),
        step("fetchinvoice", json!({"invoice":invalid})),
    ]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    assert!(adapter
        .fetch_bolt12(&offer, [7; 32], expectation(), observation())
        .await
        .is_err());
    finish(worker).await;
}

#[tokio::test]
async fn inspected_invoice_cannot_cross_accounts_or_overflow_total_reservation() {
    let (config, worker) = server(handshake()).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let mut invoice = adapter
        .inspect_bolt11(&bolt11(), expectation(), observation())
        .unwrap();
    let mut oversized = request();
    oversized.fee_limit_msat = i64::MAX as u64;
    assert!(matches!(
        adapter.dispatch(&invoice, oversized, observation()).await,
        Err(AdapterError::InvalidInput)
    ));
    invoice.account.node_id = pubkey(3);
    assert!(matches!(
        adapter.dispatch(&invoice, request(), observation()).await,
        Err(AdapterError::InvalidInput)
    ));
    assert!(matches!(
        adapter
            .lookup(&invoice, request().operation, observation())
            .await,
        Err(AdapterError::InvalidInput)
    ));
    finish(worker).await;
}

#[tokio::test]
async fn oversized_native_response_is_rejected_without_payment() {
    let mut steps = handshake();
    steps.push(step("getinfo", info()));
    steps.push(step(
        "listpays",
        json!({"pays": [], "padding":"x".repeat(MAX_RESPONSE_BYTES)}),
    ));
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let invoice = adapter
        .inspect_bolt11(&bolt11(), expectation(), observation())
        .unwrap();
    assert!(matches!(
        adapter.dispatch(&invoice, request(), observation()).await,
        Err(AdapterError::Uncertain)
    ));
    finish(worker).await;
}

fn incoming_row(invoice: &str, bolt12: bool) -> Value {
    let mut value = json!({"payment_hash":hex::encode(hash()), "status":"paid",
        "amount_msat":2000,"amount_received_msat":2010,"paid_at":1_700_000_100u64,
        "payment_preimage":hex::encode(preimage())});
    value[if bolt12 { "bolt12" } else { "bolt11" }] = json!(invoice);
    value
}

#[tokio::test]
async fn receiving_exact_lookup_preserves_received_amount_and_native_precision() {
    let text = bolt11_for(1);
    let mut steps = handshake();
    steps.push(step("getinfo", info()));
    steps.push(Step {
        path: "listinvoices",
        body: Some(json!({"payment_hash":hex::encode(hash())})),
        reply: Some(json!({"invoices":[incoming_row(&text,false)]})),
    });
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let result = adapter
        .lookup_incoming(hash(), observation())
        .await
        .unwrap();
    match result {
        IncomingStatus::Settled(proof) => {
            assert!(proof.account == account());
            assert!(proof.invoice_digest == <[u8; 32]>::from(Sha256::digest(text.as_bytes())));
            assert!(proof.payment_hash == hash());
            assert!(proof.amount_received_msat == 2010);
            assert!(proof.settled_earliest_ms == 1_700_000_100_000);
            assert!(proof.settled_latest_ms == 1_700_000_100_999);
            assert!(proof.payment_id.is_none());
        }
        _ => panic!("missing native receiving proof"),
    }
    finish(worker).await;
}

#[tokio::test]
async fn receiving_bolt12_id_requires_exact_signed_note_and_refuses_onchain_payment() {
    let (_, text) = bolt12([8; 32]);
    let mut valid = incoming_row(&text, true);
    valid["invreq_payer_note"] = json!(hex::encode([8; 32]));
    let mut wrong_note = valid.clone();
    wrong_note["invreq_payer_note"] = json!(hex::encode([9; 32]));
    let mut onchain = valid.clone();
    onchain["paid_outpoint"] = json!({"txid":"fixture","outnum":0});
    let mut steps = handshake();
    for reply in [valid, wrong_note, onchain] {
        steps.push(step("getinfo", info()));
        steps.push(step("listinvoices", json!({"invoices":[reply]})));
    }
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    assert!(
        matches!(adapter.lookup_incoming(hash(),observation()).await.unwrap(),
        IncomingStatus::Settled(IncomingSettlement {payment_id:Some(id),..}) if id == [8;32])
    );
    assert!(matches!(
        adapter.lookup_incoming(hash(), observation()).await,
        Err(AdapterError::InvoiceMismatch)
    ));
    assert!(matches!(
        adapter
            .lookup_incoming(hash(), observation())
            .await
            .unwrap(),
        IncomingStatus::Unknown
    ));
    finish(worker).await;
}

#[tokio::test]
async fn receiving_ambiguous_mismatched_and_contradictory_evidence_cannot_authorize() {
    let valid = incoming_row(&bolt11_for(1), false);
    let mut bad_proof = valid.clone();
    bad_proof["payment_preimage"] = json!(hex::encode([4; 32]));
    let mut wrong_hash = valid.clone();
    wrong_hash["payment_hash"] = json!(hex::encode([7; 32]));
    let mut pending = valid.clone();
    pending["status"] = json!("unpaid");
    let mut overflow = valid.clone();
    overflow["amount_received_msat"] = json!(u64::MAX);
    let mut wrong_payee = valid.clone();
    wrong_payee["bolt11"] = json!(bolt11());
    let replies = [
        vec![],
        vec![valid.clone(), valid],
        vec![bad_proof],
        vec![wrong_hash],
        vec![pending],
        vec![overflow],
        vec![wrong_payee],
    ];
    let mut steps = handshake();
    for rows in &replies {
        steps.push(step("getinfo", info()));
        steps.push(step("listinvoices", json!({"invoices":rows})));
    }
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    for _ in replies {
        assert!(!matches!(
            adapter.lookup_incoming(hash(), observation()).await,
            Ok(IncomingStatus::Settled(_))
        ));
    }
    finish(worker).await;
}

#[tokio::test]
async fn historical_invoice_restore_requires_exact_durable_binding_and_cannot_pay_expired() {
    let (config, worker) = server(handshake()).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let text = bolt11();
    let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
    let expected = InvoiceExpectation {
        expected_payment_hash: Some(hash()),
        ..expectation()
    };
    let restored = adapter.restore_for_lookup(&text, digest, expected).unwrap();
    let expired = Observation {
        now_ms: 1_700_010_000_000,
        uncertainty_ms: 0,
    };
    assert!(adapter.inspect_bolt11(&text, expected, expired).is_err());
    assert!(adapter
        .restore_for_lookup(&text, [9; 32], expected)
        .is_err());
    assert!(matches!(
        adapter.dispatch(&restored, request(), expired).await,
        Err(AdapterError::InvalidInput)
    ));
    finish(worker).await;
}

fn receiving_handshake() -> Vec<Step> {
    vec![
        step("getinfo", info()),
        step(
            "help",
            json!({"help":[{"command":"listinvoices [payment_hash] [index] [start] [limit]"}]}),
        ),
    ]
}
fn updates_help() -> Step {
    step(
        "help",
        json!({"help":[{"command":"listinvoices [payment_hash] [index] [start] [limit]"}]}),
    )
}
#[tokio::test]
async fn receiving_discovery_is_bounded_indexed_signed_and_requires_separate_exact_lookup() {
    let (_, text) = bolt12([8; 32]);
    let mut row = incoming_row(&text, true);
    row["invreq_payer_note"] = json!(hex::encode([8; 32]));
    row["updated_index"] = json!(7);
    row["created_index"] = json!(2);
    let mut steps = receiving_handshake();
    steps.push(step("getinfo", info()));
    steps.push(updates_help());
    steps.push(Step {
        path: "listinvoices",
        body: Some(json!({"index":"updated","start":4,"limit":16})),
        reply: Some(json!({"invoices":[row.clone()]})),
    });
    steps.push(step("getinfo", info()));
    steps.push(step("getinfo", info()));
    steps.push(Step {
        path: "listinvoices",
        body: Some(json!({"payment_hash":hex::encode(hash())})),
        reply: Some(json!({"invoices":[row]})),
    });
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect_receiving(config, account())
        .await
        .unwrap();
    let page = adapter.incoming_updates(4).await.unwrap();
    assert!(page.account == account() && page.next_index == 8 && page.rows_seen == 1);
    assert!(
        page.candidates.len() == 1
            && page.candidates[0].payment_id == [8; 32]
            && page.candidates[0].payment_hash == hash()
    );
    assert!(matches!(
        adapter
            .lookup_incoming(page.candidates[0].payment_hash, observation())
            .await
            .unwrap(),
        IncomingStatus::Settled(_)
    ));
    let restored = adapter
        .restore_for_lookup(
            &bolt11(),
            Sha256::digest(bolt11().as_bytes()).into(),
            InvoiceExpectation {
                expected_payment_hash: Some(hash()),
                ..expectation()
            },
        )
        .unwrap();
    assert!(matches!(
        adapter.dispatch(&restored, request(), observation()).await,
        Err(AdapterError::Unsupported)
    ));
    assert!(matches!(
        adapter
            .fetch_bolt12("invalid", [1; 32], expectation(), observation())
            .await,
        Err(AdapterError::Unsupported)
    ));
    finish(worker).await;
}
#[tokio::test]
async fn malformed_or_unbound_updated_pages_never_yield_a_cursor_advance() {
    let (_, text) = bolt12([8; 32]);
    let mut valid = incoming_row(&text, true);
    valid["invreq_payer_note"] = json!(hex::encode([8; 32]));
    valid["updated_index"] = json!(7);
    valid["created_index"] = json!(2);
    for case in 0..5 {
        let mut row = valid.clone();
        match case {
            0 => row["updated_index"] = json!(3),
            1 => row["invreq_payer_note"] = json!(hex::encode([9; 32])),
            2 => row["payment_hash"] = json!(hex::encode([1; 32])),
            3 => row["created_index"] = json!(0),
            _ => row["updated_index"] = json!(i64::MAX as u64),
        }
        let mut steps = receiving_handshake();
        steps.push(step("getinfo", info()));
        steps.push(updates_help());
        steps.push(step("listinvoices", json!({"invoices":[row]})));
        let (config, worker) = server(steps).await;
        let adapter = ClnFoundation::connect_receiving(config, account())
            .await
            .unwrap();
        assert!(adapter.incoming_updates(4).await.is_err());
        finish(worker).await;
    }
}
#[tokio::test]
async fn empty_updates_keep_cursor_and_unrelated_native_metadata_is_not_a_payment_candidate() {
    let mut steps = receiving_handshake();
    for rows in [
        json!([]),
        json!([{"created_index":2,"updated_index":10,"status":"expired"}]),
    ] {
        steps.push(step("getinfo", info()));
        steps.push(updates_help());
        steps.push(step("listinvoices", json!({"invoices":rows})));
        steps.push(step("getinfo", info()));
    }
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect_receiving(config, account())
        .await
        .unwrap();
    let empty = adapter.incoming_updates(4).await.unwrap();
    assert!(empty.next_index == 4 && empty.candidates.is_empty());
    let expired = adapter.incoming_updates(4).await.unwrap();
    assert!(expired.next_index == 11 && expired.candidates.is_empty());
    finish(worker).await;
}

#[tokio::test]
async fn construction_proof_is_lookup_only_and_exactly_bound() {
    let invoice = bolt11();
    let mut wrong = row(&invoice, "complete");
    wrong["label"] = json!(hex::encode([99; 16]));
    let mut steps = handshake();
    steps.extend([
        step("getinfo", info()),
        step("listpays", json!({"pays":[wrong]})),
        step("getinfo", info()),
        step("listpays", json!({"pays":[row(&invoice,"complete"),row(&invoice,"complete")]})),
        step("getinfo", info()),
        step("listpays", json!({"pays":[row(&invoice,"complete")]})),
    ]);
    let (config, worker) = server(steps).await;
    let adapter = ClnFoundation::connect(config, account()).await.unwrap();
    let prepared = adapter.inspect_bolt11(&invoice, expectation(), observation()).unwrap();
    for _ in 0..2 {
        assert!(adapter.lookup_construction_proof(&prepared, request().operation, observation()).await.unwrap().is_none());
    }
    let proof = adapter.lookup_construction_proof(&prepared, request().operation, observation()).await.unwrap().unwrap();
    assert_eq!(*proof.preimage(), preimage());
    assert_eq!(proof.settlement().fee_msat, 50);
    assert_eq!(proof.settlement().invoice_digest, prepared.digest());
    finish(worker).await;
}

#[tokio::test]
async fn restored_construction_invoice_still_requires_exact_offer_and_payer_note() {
    let (offer, invoice) = bolt12([7;32]);
    let (config, worker) = server(handshake()).await;
    let adapter=ClnFoundation::connect(config,account()).await.unwrap();
    let digest: [u8;32]=Sha256::digest(invoice.as_bytes()).into();
    let expected=InvoiceExpectation{expected_payment_hash:Some(hash()),..expectation()};
    assert!(adapter.restore_bolt12_for_offer_lookup(&invoice,digest,&offer,[7;32],expected).is_ok());
    assert!(adapter.restore_bolt12_for_offer_lookup(&invoice,digest,&offer,[8;32],expected).is_err());
    let (other,_) = bolt12([8;32]);
    // Same offer may issue several notes; changed note still cannot restore.
    assert!(adapter.restore_bolt12_for_offer_lookup(&invoice,digest,&other,[8;32],expected).is_err());
    finish(worker).await;
}
