//! Strict Rust-only paid-circuit adapter. All spending requires an externally
//! persisted one-time dispatch claim and accepted invoice/quote/round binding.
//! This module never retries a pay request. Missing, ambiguous or unbound native
//! records remain UNKNOWN; a transport/RPC error never establishes failure.
//! CLN's native maxfee and label are required through help introspection. HTTP
//! fixtures establish request/validation behavior, not real Lightning settlement.

use super::ClnConfig;
use bech32::{primitives::decode::CheckedHrpstring, Hrp, NoChecksum};
use lightning::{
    bitcoin::{constants::ChainHash, secp256k1::PublicKey, Network},
    offers::{invoice::Bolt12Invoice, offer::Offer},
};
use lightning_invoice::{Bolt11Invoice, Currency};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{str::FromStr, time::Duration};
use thiserror::Error;

// Matches the v1 invoice artifact ceiling. The response ceiling accommodates
// one invoice plus bounded RPC metadata; it is not a production load claim.
const MAX_INVOICE_BYTES: usize = 2048;
const MAX_RESPONSE_BYTES: usize = 65536;
type Result<T> = std::result::Result<T, AdapterError>;

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("invalid native wallet input")]
    InvalidInput,
    #[error("unsupported native wallet capability")]
    Unsupported,
    #[error("native wallet transport or RPC outcome is uncertain")]
    Uncertain,
    #[error("invalid native wallet response")]
    InvalidResponse,
    #[error("native wallet account changed")]
    AccountMismatch,
    #[error("native invoice binding or validity mismatch")]
    InvoiceMismatch,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Chain {
    Bitcoin,
    Testnet,
    Regtest,
    Signet,
}
impl Chain {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "bitcoin" => Ok(Self::Bitcoin),
            "testnet" => Ok(Self::Testnet),
            "regtest" => Ok(Self::Regtest),
            "signet" => Ok(Self::Signet),
            _ => Err(AdapterError::Unsupported),
        }
    }
    fn network(self) -> Network {
        match self {
            Self::Bitcoin => Network::Bitcoin,
            Self::Testnet => Network::Testnet,
            Self::Regtest => Network::Regtest,
            Self::Signet => Network::Signet,
        }
    }
    fn currency(self) -> Currency {
        match self {
            Self::Bitcoin => Currency::Bitcoin,
            Self::Testnet => Currency::BitcoinTestnet,
            Self::Regtest => Currency::Regtest,
            Self::Signet => Currency::Signet,
        }
    }
    fn code(self) -> u8 {
        match self {
            Self::Bitcoin => 0,
            Self::Testnet => 1,
            Self::Regtest => 2,
            Self::Signet => 3,
        }
    }
}

/// Native node identity and chain, never a URL/rune/caller label. No Debug:
/// account associations must not appear in routine diagnostic logs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NativeAccount {
    pub node_id: [u8; 33],
    pub chain: Chain,
}
impl NativeAccount {
    pub fn fingerprint(self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"lni/lightning-account/v1\0");
        hash.update([self.chain.code()]);
        hash.update(self.node_id);
        hash.finalize().into()
    }
}

#[derive(Clone, Copy)]
pub struct Observation {
    pub now_ms: i64,
    pub uncertainty_ms: u64,
}
impl Observation {
    fn latest(self) -> Result<i64> {
        if self.now_ms < 0 {
            return Err(AdapterError::InvalidInput);
        }
        self.now_ms
            .checked_add(
                i64::try_from(self.uncertainty_ms).map_err(|_| AdapterError::InvalidInput)?,
            )
            .ok_or(AdapterError::InvalidInput)
    }
}

/// Expected recipient is mandatory for BOLT11. BOLT12 may retain receiver
/// privacy by verifying the exact accepted offer and its native signing rules.
#[derive(Clone, Copy)]
pub struct InvoiceExpectation {
    pub amount_msat: u64,
    pub expected_payee: Option<[u8; 33]>,
    pub expected_payment_hash: Option<[u8; 32]>,
}

/// Native invoice verification only. El Tor's issuer/quote/client/round binding
/// must separately authenticate this exact digest before ledger reservation.
#[derive(Clone)]
pub struct ValidatedInvoice {
    account: NativeAccount,
    text: String,
    digest: [u8; 32],
    payment_hash: [u8; 32],
    amount_msat: u64,
    payee: [u8; 33],
    expires_at_ms: i64,
    amountless: bool,
}
impl ValidatedInvoice {
    pub fn account(&self) -> NativeAccount {
        self.account
    }
    pub fn invoice(&self) -> &str {
        &self.text
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn payment_hash(&self) -> [u8; 32] {
        self.payment_hash
    }
    pub fn amount_msat(&self) -> u64 {
        self.amount_msat
    }
    pub fn payee(&self) -> [u8; 33] {
        self.payee
    }
    pub fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
}

#[derive(Clone, Copy)]
pub struct DispatchRequest {
    pub operation: [u8; 16],
    pub fee_limit_msat: u64,
    pub retry_for_seconds: u16,
    pub pay_before_ms: i64,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NativeSettlement {
    pub account: NativeAccount,
    pub operation: [u8; 16],
    pub invoice_digest: [u8; 32],
    pub payment_hash: [u8; 32],
    pub amount_msat: u64,
    pub fee_msat: u64,
    /// CLN listpays completed_at is seconds. Preserve its precision interval,
    /// rather than presenting the floored timestamp as exact milliseconds.
    pub settled_earliest_ms: i64,
    pub settled_latest_ms: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub enum PaymentStatus {
    Unknown,
    Pending,
    Settled(NativeSettlement),
    FailedFinal,
}

/// Ephemeral native proof for the existing paid-construction input. Deliberately
/// neither Debug, Clone nor serializable: financial records retain the exact
/// invoice and sanitized settlement, never the preimage. Recover it by an exact
/// read-only lookup, not by repeating pay.
pub struct ConstructionPaymentProof {
    settlement: NativeSettlement,
    preimage: [u8; 32],
}
impl ConstructionPaymentProof {
    pub fn settlement(&self) -> &NativeSettlement {
        &self.settlement
    }
    pub fn preimage(&self) -> &[u8; 32] {
        &self.preimage
    }
}
impl Drop for ConstructionPaymentProof {
    fn drop(&mut self) {
        self.preimage.fill(0);
    }
}

/// Evidence from an exact account-owned Lightning invoice lookup. Amount is the
/// amount actually received, not the requested invoice amount. The ledger must
/// validate its accepted amount/window/hash-or-ID binding before authorization.
#[derive(Clone, PartialEq, Eq)]
pub struct IncomingSettlement {
    pub account: NativeAccount,
    pub invoice_digest: [u8; 32],
    pub payment_hash: [u8; 32],
    pub amount_received_msat: u64,
    pub payment_id: Option<[u8; 32]>,
    pub settled_earliest_ms: i64,
    pub settled_latest_ms: i64,
}
#[derive(Clone, PartialEq, Eq)]
pub enum IncomingStatus {
    Unknown,
    Pending,
    Expired,
    Settled(IncomingSettlement),
}

pub const INCOMING_UPDATE_PAGE_LIMIT: usize = 16;
#[derive(Clone, PartialEq, Eq)]
pub struct IncomingInvoiceCandidate {
    pub payment_id: [u8; 32],
    pub payment_hash: [u8; 32],
    pub invoice_digest: [u8; 32],
    pub updated_index: u64,
}
#[derive(Clone, PartialEq, Eq)]
pub struct IncomingUpdatePage {
    pub account: NativeAccount,
    pub from_index: u64,
    pub next_index: u64,
    pub rows_seen: usize,
    pub candidates: Vec<IncomingInvoiceCandidate>,
}

/// Does not implement Debug and never includes native JSON/error bodies in errors.
pub struct ClnFoundation {
    base: reqwest::Url,
    client: reqwest::Client,
    account: NativeAccount,
    outgoing_ready: bool,
}

impl ClnFoundation {
    /// Verify the actual account and source-observed capabilities before exposing
    /// payment support. This performs read-only RPCs; no invoice acquisition/pay.
    pub async fn connect(config: ClnConfig, expected: NativeAccount) -> Result<Self> {
        Self::connect_for(config, expected, true).await
    }
    /// Receiving-only capability requires no wallet payment or fetch permission.
    pub async fn connect_receiving(config: ClnConfig, expected: NativeAccount) -> Result<Self> {
        Self::connect_for(config, expected, false).await
    }
    async fn connect_for(
        config: ClnConfig,
        expected: NativeAccount,
        outgoing_ready: bool,
    ) -> Result<Self> {
        PublicKey::from_slice(&expected.node_id).map_err(|_| AdapterError::InvalidInput)?;
        let mut base = reqwest::Url::parse(&config.url).map_err(|_| AdapterError::InvalidInput)?;
        if !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
        {
            return Err(AdapterError::InvalidInput);
        }
        if base.scheme() != "https"
            && !(base.scheme() == "http"
                && base
                    .host_str()
                    .and_then(|s| s.trim_matches(['[', ']']).parse::<std::net::IpAddr>().ok())
                    .map_or(false, |ip| ip.is_loopback()))
        {
            return Err(AdapterError::InvalidInput);
        }
        let insecure_tls = config.accept_invalid_certs == Some(true);
        if insecure_tls
            && (outgoing_ready
                || !receiving_test_tls_endpoint(&base)
                || config
                    .socks5_proxy
                    .as_deref()
                    .map_or(false, |p| !p.is_empty()))
        {
            return Err(AdapterError::InvalidInput);
        }
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        let mut headers = reqwest::header::HeaderMap::new();
        if config.rune.is_empty() {
            return Err(AdapterError::InvalidInput);
        }
        let mut rune = reqwest::header::HeaderValue::from_str(&config.rune)
            .map_err(|_| AdapterError::InvalidInput)?;
        rune.set_sensitive(true);
        headers.insert("Rune", rune);
        let timeout = config.http_timeout.unwrap_or(60);
        if timeout <= 0 {
            return Err(AdapterError::InvalidInput);
        }
        let mut builder = reqwest::Client::builder()
            .danger_accept_invalid_certs(insecure_tls)
            .no_proxy()
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .timeout(Duration::from_secs(timeout as u64));
        if let Some(proxy) = config.socks5_proxy.as_deref().filter(|p| !p.is_empty()) {
            builder =
                builder.proxy(reqwest::Proxy::all(proxy).map_err(|_| AdapterError::InvalidInput)?);
        }
        let adapter = Self {
            base,
            client: builder.build().map_err(|_| AdapterError::InvalidInput)?,
            account: expected,
            outgoing_ready,
        };
        adapter.check_account().await?;
        if outgoing_ready {
            for (command, required) in [
                ("pay", &["label", "maxfee"][..]),
                ("listpays", &["payment_hash"][..]),
                ("fetchinvoice", &["offer", "amount_msat", "payer_note"][..]),
            ] {
                adapter.require_command(command, required).await?;
            }
        } else {
            adapter
                .require_command("listinvoices", &["payment_hash", "index", "start", "limit"])
                .await?;
        }
        Ok(adapter)
    }

    async fn require_command(&self, command: &str, required: &[&str]) -> Result<()> {
        let help = self.rpc("help", json!({"command":command})).await?;
        let rows = help
            .get("help")
            .and_then(Value::as_array)
            .ok_or(AdapterError::Unsupported)?;
        if rows.len() != 1 {
            return Err(AdapterError::Unsupported);
        }
        let text = rows[0]
            .get("command")
            .and_then(Value::as_str)
            .ok_or(AdapterError::Unsupported)?;
        let tokens: Vec<_> = text
            .split_whitespace()
            .map(|t| t.trim_matches(['[', ']']))
            .collect();
        if tokens.first().copied() != Some(command)
            || required.iter().any(|field| !tokens.contains(field))
        {
            return Err(AdapterError::Unsupported);
        }
        Ok(())
    }
    pub fn account(&self) -> NativeAccount {
        self.account
    }

    pub async fn check_account(&self) -> Result<()> {
        #[derive(Deserialize)]
        struct Info {
            id: String,
            network: String,
        }
        let info: Info = serde_json::from_value(self.rpc("getinfo", json!({})).await?)
            .map_err(|_| AdapterError::InvalidResponse)?;
        let node = PublicKey::from_str(&info.id)
            .map_err(|_| AdapterError::InvalidResponse)?
            .serialize();
        if (NativeAccount {
            node_id: node,
            chain: Chain::parse(&info.network)?,
        }) != self.account
        {
            return Err(AdapterError::AccountMismatch);
        }
        Ok(())
    }

    pub fn inspect_bolt11(
        &self,
        text: &str,
        expected: InvoiceExpectation,
        observation: Observation,
    ) -> Result<ValidatedInvoice> {
        canonical_text(text)?;
        let invoice = Bolt11Invoice::from_str(text).map_err(|_| AdapterError::InvoiceMismatch)?;
        invoice
            .check_signature()
            .map_err(|_| AdapterError::InvoiceMismatch)?;
        let payee = invoice.recover_payee_pub_key().serialize();
        if expected.expected_payee != Some(payee)
            || invoice.currency() != self.account.chain.currency()
            || invoice
                .features()
                .map_or(false, |f| f.requires_unknown_bits())
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        let amountless = invoice.amount_milli_satoshis().is_none();
        let amount = invoice
            .amount_milli_satoshis()
            .unwrap_or(expected.amount_msat);
        let hash = decode32(&invoice.payment_hash().to_string())?;
        let created = milliseconds(invoice.duration_since_epoch())?;
        let expires = milliseconds(invoice.expires_at().ok_or(AdapterError::InvoiceMismatch)?)?;
        validate_invoice_values(amount, hash, created, expires, expected, observation)?;
        Ok(ValidatedInvoice {
            account: self.account,
            text: text.to_owned(),
            digest: Sha256::digest(text.as_bytes()).into(),
            payment_hash: hash,
            amount_msat: amount,
            payee,
            expires_at_ms: expires,
            amountless,
        })
    }

    /// Reconstruct an exact previously inspected artifact for read-only recovery.
    /// The durable intent supplies its digest/hash/amount; no new offer binding or
    /// freshness is inferred here. Dispatch still rejects expired artifacts.
    pub fn restore_for_lookup(
        &self,
        text: &str,
        expected_digest: [u8; 32],
        expected: InvoiceExpectation,
    ) -> Result<ValidatedInvoice> {
        canonical_text(text)?;
        let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
        if digest != expected_digest
            || expected.expected_payment_hash.is_none()
            || expected.amount_msat == 0
            || expected.amount_msat > i64::MAX as u64
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        let (hash, amount, payee, expires, amountless) = if text.starts_with("lni1") {
            let invoice = parse_bolt12(text)?;
            if invoice.chain() != ChainHash::using_genesis_block(self.account.chain.network()) {
                return Err(AdapterError::InvoiceMismatch);
            }
            (
                invoice.payment_hash().0,
                invoice.amount_msats(),
                invoice.signing_pubkey().serialize(),
                milliseconds(
                    invoice
                        .created_at()
                        .checked_add(invoice.relative_expiry())
                        .ok_or(AdapterError::InvoiceMismatch)?,
                )?,
                false,
            )
        } else {
            let invoice =
                Bolt11Invoice::from_str(text).map_err(|_| AdapterError::InvoiceMismatch)?;
            invoice
                .check_signature()
                .map_err(|_| AdapterError::InvoiceMismatch)?;
            if invoice.currency() != self.account.chain.currency()
                || invoice
                    .features()
                    .map_or(false, |f| f.requires_unknown_bits())
            {
                return Err(AdapterError::InvoiceMismatch);
            }
            (
                decode32(&invoice.payment_hash().to_string())?,
                invoice
                    .amount_milli_satoshis()
                    .unwrap_or(expected.amount_msat),
                invoice.recover_payee_pub_key().serialize(),
                milliseconds(invoice.expires_at().ok_or(AdapterError::InvoiceMismatch)?)?,
                invoice.amount_milli_satoshis().is_none(),
            )
        };
        if Some(hash) != expected.expected_payment_hash
            || amount != expected.amount_msat
            || expected.expected_payee.map_or(false, |p| p != payee)
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        Ok(ValidatedInvoice {
            account: self.account,
            text: text.to_owned(),
            digest,
            payment_hash: hash,
            amount_msat: amount,
            payee,
            expires_at_ms: expires,
            amountless,
        })
    }

    /// Fetch-only native BOLT12 acquisition. No pay RPC occurs here. The signed
    /// invoice must preserve the accepted offer, chain, amount and exact note.
    pub async fn fetch_bolt12(
        &self,
        offer_text: &str,
        payment_id: [u8; 32],
        expected: InvoiceExpectation,
        observation: Observation,
    ) -> Result<ValidatedInvoice> {
        if !self.outgoing_ready {
            return Err(AdapterError::Unsupported);
        }
        canonical_text(offer_text)?;
        let offer = Offer::from_str(offer_text).map_err(|_| AdapterError::InvoiceMismatch)?;
        if offer.to_string() != offer_text
            || offer.offer_features().requires_unknown_bits()
            || !offer.chains().contains(&ChainHash::using_genesis_block(
                self.account.chain.network(),
            ))
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        if payment_id == [0; 32]
            || expected.amount_msat == 0
            || expected.amount_msat > i64::MAX as u64
        {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        let note = hex::encode(payment_id);
        let reply=self.rpc("fetchinvoice",json!({"offer":offer_text,"amount_msat":format!("{}msat",expected.amount_msat),"payer_note":note,"timeout":30})).await?;
        let text = reply
            .get("invoice")
            .and_then(Value::as_str)
            .ok_or(AdapterError::InvalidResponse)?;
        canonical_text(text)?;
        let invoice = parse_bolt12(text)?;
        let payee = invoice.signing_pubkey().serialize();
        if invoice.offer_id() != Some(offer.id())
            || invoice.chain() != ChainHash::using_genesis_block(self.account.chain.network())
            || invoice.payer_note().map(|n| n.to_string()) != Some(note)
            || expected.expected_payee.map_or(false, |p| p != payee)
            || invoice.invoice_features().requires_unknown_bits()
            || invoice.invoice_request_features().requires_unknown_bits()
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        let created = milliseconds(invoice.created_at())?;
        let expires = milliseconds(
            invoice
                .created_at()
                .checked_add(invoice.relative_expiry())
                .ok_or(AdapterError::InvoiceMismatch)?,
        )?;
        validate_invoice_values(
            invoice.amount_msats(),
            invoice.payment_hash().0,
            created,
            expires,
            expected,
            observation,
        )?;
        Ok(ValidatedInvoice {
            account: self.account,
            text: text.to_owned(),
            digest: Sha256::digest(text.as_bytes()).into(),
            payment_hash: invoice.payment_hash().0,
            amount_msat: invoice.amount_msats(),
            payee,
            expires_at_ms: expires,
            amountless: false,
        })
    }

    /// Restore an exact historical invoice for a pinned offer/note. Expiry may
    /// prevent new dispatch but never prevents read-only reconciliation.
    pub fn restore_bolt12_for_offer_lookup(
        &self,
        text: &str,
        expected_digest: [u8; 32],
        offer_text: &str,
        payment_id: [u8; 32],
        expected: InvoiceExpectation,
    ) -> Result<ValidatedInvoice> {
        if !self.outgoing_ready || payment_id == [0; 32] {
            return Err(AdapterError::Unsupported);
        }
        canonical_text(offer_text)?;
        let offer = Offer::from_str(offer_text).map_err(|_| AdapterError::InvoiceMismatch)?;
        let invoice = parse_bolt12(text)?;
        if offer.to_string() != offer_text
            || offer.offer_features().requires_unknown_bits()
            || !offer.chains().contains(&ChainHash::using_genesis_block(self.account.chain.network()))
            || invoice.offer_id() != Some(offer.id())
            || invoice.payer_note().map(|s| s.to_string()) != Some(hex::encode(payment_id))
            || invoice.invoice_features().requires_unknown_bits()
            || invoice.invoice_request_features().requires_unknown_bits()
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        self.restore_for_lookup(text, expected_digest, expected)
    }

    /// Invoke at most once under a consumed durable ledger permit. Existing
    /// native records are recovered first; pay is never repeated on uncertainty.
    pub async fn dispatch(
        &self,
        invoice: &ValidatedInvoice,
        request: DispatchRequest,
        observation: Observation,
    ) -> Result<PaymentStatus> {
        if !self.outgoing_ready {
            return Err(AdapterError::Unsupported);
        }
        if invoice.account != self.account
            || request.operation == [0; 16]
            || request.fee_limit_msat > i64::MAX as u64
            || invoice
                .amount_msat
                .checked_add(request.fee_limit_msat)
                .map_or(true, |n| n > i64::MAX as u64)
            || request.retry_for_seconds == 0
            || request.pay_before_ms > invoice.expires_at_ms
            || request.pay_before_ms <= observation.latest()?
            || i64::from(request.retry_for_seconds) * 1000
                > request.pay_before_ms - observation.latest()?
        {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        let records = self.records(invoice.payment_hash).await?;
        if !records.is_empty() {
            return self.interpret(invoice, request.operation, &records, observation);
        }
        let mut args = json!({"bolt11":invoice.text,"maxfee":format!("{}msat",request.fee_limit_msat),"label":hex::encode(request.operation),"retry_for":request.retry_for_seconds});
        if invoice.amountless {
            args["amount_msat"] = json!(format!("{}msat", invoice.amount_msat));
        }
        // Any error after the durable claim remains uncertain. Never inspect an
        // error string to infer failure or issue another payment automatically.
        self.rpc("pay", args).await?;
        self.lookup(invoice, request.operation, observation).await
    }

    pub async fn lookup(
        &self,
        invoice: &ValidatedInvoice,
        operation: [u8; 16],
        observation: Observation,
    ) -> Result<PaymentStatus> {
        if invoice.account != self.account || operation == [0; 16] {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        self.interpret(
            invoice,
            operation,
            &self.records(invoice.payment_hash).await?,
            observation,
        )
    }

    /// Read-only proof recovery. It applies exactly the same single-record,
    /// native account, label, invoice, amount, time and hash checks as lookup.
    /// Pending/failed/ambiguous results yield no construction proof.
    pub async fn lookup_construction_proof(
        &self,
        invoice: &ValidatedInvoice,
        operation: [u8; 16],
        observation: Observation,
    ) -> Result<Option<ConstructionPaymentProof>> {
        if !self.outgoing_ready {
            return Err(AdapterError::Unsupported);
        }
        if invoice.account != self.account || operation == [0; 16] {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        let rows = self.records(invoice.payment_hash).await?;
        let PaymentStatus::Settled(settlement) =
            self.interpret(invoice, operation, &rows, observation)?
        else {
            return Ok(None);
        };
        let preimage = decode32(rows[0].get("preimage").and_then(Value::as_str)
            .ok_or(AdapterError::InvalidResponse)?)?;
        Ok(Some(ConstructionPaymentProof { settlement, preimage }))
    }

    /// Read-only exact native receiving lookup. Public payer claims and a paid
    /// status string alone cannot create evidence. On-chain paid_outpoint is
    /// deliberately unsupported by the paid-circuit Lightning contract.
    pub async fn lookup_incoming(
        &self,
        payment_hash: [u8; 32],
        observation: Observation,
    ) -> Result<IncomingStatus> {
        if payment_hash == [0; 32] {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        let reply = self
            .rpc(
                "listinvoices",
                json!({"payment_hash":hex::encode(payment_hash)}),
            )
            .await?;
        let records = reply
            .get("invoices")
            .and_then(Value::as_array)
            .ok_or(AdapterError::InvalidResponse)?;
        if records.len() != 1 {
            return Ok(IncomingStatus::Unknown);
        }
        let row = &records[0];
        if row.get("payment_hash").and_then(Value::as_str)
            != Some(hex::encode(payment_hash).as_str())
            || row.get("paid_outpoint").map_or(false, |v| !v.is_null())
        {
            return Ok(IncomingStatus::Unknown);
        }
        let texts: Vec<_> = [
            row.get("bolt11").and_then(Value::as_str),
            row.get("bolt12").and_then(Value::as_str),
        ]
        .into_iter()
        .flatten()
        .collect();
        if texts.len() != 1 {
            return Ok(IncomingStatus::Unknown);
        }
        let text = texts[0];
        canonical_text(text)?;
        let payment_id = if row.get("bolt11").and_then(Value::as_str).is_some() {
            let invoice =
                Bolt11Invoice::from_str(text).map_err(|_| AdapterError::InvoiceMismatch)?;
            invoice
                .check_signature()
                .map_err(|_| AdapterError::InvoiceMismatch)?;
            if invoice.currency() != self.account.chain.currency()
                || invoice.recover_payee_pub_key().serialize() != self.account.node_id
                || decode32(&invoice.payment_hash().to_string())? != payment_hash
            {
                return Err(AdapterError::InvoiceMismatch);
            }
            None
        } else {
            let invoice = parse_bolt12(text)?;
            if invoice.chain() != ChainHash::using_genesis_block(self.account.chain.network())
                || invoice.payment_hash().0 != payment_hash
            {
                return Err(AdapterError::InvoiceMismatch);
            }
            let native_note = row.get("invreq_payer_note").and_then(Value::as_str);
            if invoice.payer_note().map(|n| n.to_string()).as_deref() != native_note {
                return Err(AdapterError::InvoiceMismatch);
            }
            // A generic note is not an El Tor ID. Only the exact canonical
            // 32-byte hex representation can bind an ID at the ledger boundary.
            native_note
                .filter(|n| {
                    n.len() == 64
                        && n.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
                .map(decode32)
                .transpose()?
        };
        match row.get("status").and_then(Value::as_str) {
            Some("unpaid") | Some("expired") => {
                if ["paid_at", "payment_preimage", "amount_received_msat"]
                    .iter()
                    .any(|k| row.get(*k).map_or(false, |v| !v.is_null()))
                {
                    return Ok(IncomingStatus::Unknown);
                }
                Ok(
                    if row.get("status").and_then(Value::as_str) == Some("unpaid") {
                        IncomingStatus::Pending
                    } else {
                        IncomingStatus::Expired
                    },
                )
            }
            Some("paid") => {
                let proof = decode32(
                    row.get("payment_preimage")
                        .and_then(Value::as_str)
                        .ok_or(AdapterError::InvalidResponse)?,
                )?;
                if <[u8; 32]>::from(Sha256::digest(proof)) != payment_hash {
                    return Err(AdapterError::InvalidResponse);
                }
                let amount = msat(row.get("amount_received_msat"))?;
                let paid = seconds(row.get("paid_at"))?;
                if amount == 0 || paid > observation.latest()? {
                    return Err(AdapterError::InvalidResponse);
                }
                Ok(IncomingStatus::Settled(IncomingSettlement {
                    account: self.account,
                    invoice_digest: Sha256::digest(text.as_bytes()).into(),
                    payment_hash,
                    amount_received_msat: amount,
                    payment_id,
                    settled_earliest_ms: paid,
                    settled_latest_ms: paid
                        .checked_add(999)
                        .ok_or(AdapterError::InvalidResponse)?,
                }))
            }
            _ => Ok(IncomingStatus::Unknown),
        }
    }

    /// Bounded UPDATED-index discovery only. A returned candidate is not settled
    /// evidence: the caller must persist its association and perform the exact
    /// hash lookup before a ledger claim. No node history is silently skipped.
    pub async fn incoming_updates(&self, from_index: u64) -> Result<IncomingUpdatePage> {
        if from_index == 0 || from_index > i64::MAX as u64 {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        self.require_command("listinvoices", &["payment_hash", "index", "start", "limit"])
            .await?;
        let reply = self
            .rpc(
                "listinvoices",
                json!({"index":"updated","start":from_index,"limit":INCOMING_UPDATE_PAGE_LIMIT}),
            )
            .await?;
        let rows = reply
            .get("invoices")
            .and_then(Value::as_array)
            .ok_or(AdapterError::InvalidResponse)?;
        if rows.len() > INCOMING_UPDATE_PAGE_LIMIT {
            return Err(AdapterError::InvalidResponse);
        }
        let mut last = None;
        let mut candidates = Vec::new();
        let mut hashes = Vec::new();
        for row in rows {
            let index = row
                .get("updated_index")
                .and_then(Value::as_u64)
                .ok_or(AdapterError::InvalidResponse)?;
            if index < from_index
                || index >= i64::MAX as u64
                || last.map_or(false, |n| index <= n)
                || row
                    .get("created_index")
                    .and_then(Value::as_u64)
                    .map_or(true, |n| n == 0)
            {
                return Err(AdapterError::InvalidResponse);
            }
            last = Some(index);
            let status = row
                .get("status")
                .and_then(Value::as_str)
                .ok_or(AdapterError::InvalidResponse)?;
            if !["unpaid", "paid", "expired"].contains(&status) {
                return Err(AdapterError::InvalidResponse);
            }
            if status != "paid" || row.get("paid_outpoint").map_or(false, |v| !v.is_null()) {
                continue;
            }
            let note = match row.get("invreq_payer_note").and_then(Value::as_str) {
                Some(n)
                    if n.len() == 64
                        && n.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
                {
                    n
                }
                _ => continue,
            };
            let payment_id = decode32(note)?;
            if payment_id == [0; 32] {
                continue;
            }
            if row.get("bolt11").map_or(false, |v| !v.is_null()) {
                return Err(AdapterError::InvoiceMismatch);
            }
            let text = row
                .get("bolt12")
                .and_then(Value::as_str)
                .ok_or(AdapterError::InvalidResponse)?;
            canonical_text(text)?;
            let invoice = parse_bolt12(text)?;
            let payment_hash = decode32(
                row.get("payment_hash")
                    .and_then(Value::as_str)
                    .ok_or(AdapterError::InvalidResponse)?,
            )?;
            if invoice.chain() != ChainHash::using_genesis_block(self.account.chain.network())
                || invoice.payment_hash().0 != payment_hash
                || invoice.payer_note().map(|n| n.to_string()).as_deref() != Some(note)
                || hashes.contains(&payment_hash)
                || invoice.amount_msats() == 0
                || invoice.amount_msats() > i64::MAX as u64
            {
                return Err(AdapterError::InvoiceMismatch);
            }
            hashes.push(payment_hash);
            candidates.push(IncomingInvoiceCandidate {
                payment_id,
                payment_hash,
                invoice_digest: Sha256::digest(text.as_bytes()).into(),
                updated_index: index,
            });
        }
        self.check_account().await?;
        Ok(IncomingUpdatePage {
            account: self.account,
            from_index,
            next_index: last.map_or(from_index, |n| n + 1),
            rows_seen: rows.len(),
            candidates,
        })
    }

    async fn records(&self, hash: [u8; 32]) -> Result<Vec<Value>> {
        let reply = self
            .rpc("listpays", json!({"payment_hash":hex::encode(hash)}))
            .await?;
        reply
            .get("pays")
            .and_then(Value::as_array)
            .cloned()
            .ok_or(AdapterError::InvalidResponse)
    }

    fn interpret(
        &self,
        invoice: &ValidatedInvoice,
        operation: [u8; 16],
        records: &[Value],
        observation: Observation,
    ) -> Result<PaymentStatus> {
        if records.len() != 1 {
            return Ok(PaymentStatus::Unknown);
        }
        let row = &records[0];
        let invoices: Vec<_> = [
            row.get("bolt11").and_then(Value::as_str),
            row.get("bolt12").and_then(Value::as_str),
        ]
        .into_iter()
        .flatten()
        .collect();
        if row.get("payment_hash").and_then(Value::as_str)
            != Some(hex::encode(invoice.payment_hash).as_str())
            || row.get("label").and_then(Value::as_str) != Some(hex::encode(operation).as_str())
            || invoices.len() != 1
            || invoices[0] != invoice.text
        {
            return Ok(PaymentStatus::Unknown);
        }
        match row.get("status").and_then(Value::as_str) {
            Some("pending") => {
                if row.get("preimage").map_or(false, |v| !v.is_null())
                    || row.get("completed_at").map_or(false, |v| !v.is_null())
                {
                    return Ok(PaymentStatus::Unknown);
                }
                Ok(PaymentStatus::Pending)
            }
            Some("failed") => {
                if row.get("preimage").map_or(false, |v| !v.is_null())
                    || row
                        .get("amount_sent_msat")
                        .map_or(false, |v| msat(Some(v)).map_or(true, |n| n != 0))
                {
                    return Ok(PaymentStatus::Unknown);
                }
                Ok(PaymentStatus::FailedFinal)
            }
            Some("complete") => {
                let amount = msat(row.get("amount_msat"))?;
                let sent = msat(row.get("amount_sent_msat"))?;
                let proof = decode32(
                    row.get("preimage")
                        .and_then(Value::as_str)
                        .ok_or(AdapterError::InvalidResponse)?,
                )?;
                if amount != invoice.amount_msat
                    || sent < amount
                    || <[u8; 32]>::from(Sha256::digest(proof)) != invoice.payment_hash
                {
                    return Err(AdapterError::InvalidResponse);
                }
                let completed = seconds(row.get("completed_at"))?;
                let created = seconds(row.get("created_at"))?;
                if completed < created || completed > observation.latest()? {
                    return Err(AdapterError::InvalidResponse);
                }
                Ok(PaymentStatus::Settled(NativeSettlement {
                    account: self.account,
                    operation,
                    invoice_digest: invoice.digest,
                    payment_hash: invoice.payment_hash,
                    amount_msat: amount,
                    fee_msat: sent - amount,
                    settled_earliest_ms: completed,
                    settled_latest_ms: completed
                        .checked_add(999)
                        .ok_or(AdapterError::InvalidResponse)?,
                }))
            }
            _ => Ok(PaymentStatus::Unknown),
        }
    }

    async fn rpc(&self, method: &str, body: Value) -> Result<Value> {
        let url = self
            .base
            .join(&format!("v1/{method}"))
            .map_err(|_| AdapterError::InvalidInput)?;
        let mut response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|_| AdapterError::Uncertain)?;
        if !response.status().is_success()
            || response
                .content_length()
                .map_or(false, |n| n > MAX_RESPONSE_BYTES as u64)
        {
            return Err(AdapterError::Uncertain);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AdapterError::Uncertain)?
        {
            if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
                return Err(AdapterError::InvalidResponse);
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| AdapterError::InvalidResponse)?;
        if value.get("code").is_some() || value.get("error").is_some() {
            return Err(AdapterError::Uncertain);
        }
        Ok(value)
    }
}

/// Explicit receiving-only test exception. Never relax outgoing verification,
/// public/DNS destinations, plaintext transport, proxies or redirects.
fn receiving_test_tls_endpoint(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }
    match url
        .host_str()
        .and_then(|h| h.parse::<std::net::IpAddr>().ok())
    {
        Some(std::net::IpAddr::V4(ip)) => {
            let octets = ip.octets();
            ip.is_loopback() || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        }
        _ => false,
    }
}

fn parse_bolt12(text: &str) -> Result<Bolt12Invoice> {
    let parsed =
        CheckedHrpstring::new::<NoChecksum>(text).map_err(|_| AdapterError::InvoiceMismatch)?;
    if parsed.hrp().as_str() != "lni" {
        return Err(AdapterError::InvoiceMismatch);
    }
    let bytes: Vec<u8> = parsed.byte_iter().collect();
    if bech32::encode::<NoChecksum>(
        Hrp::parse("lni").map_err(|_| AdapterError::InvalidInput)?,
        &bytes,
    )
    .map_err(|_| AdapterError::InvoiceMismatch)?
        != text
    {
        return Err(AdapterError::InvoiceMismatch);
    }
    let invoice = Bolt12Invoice::try_from(bytes).map_err(|_| AdapterError::InvoiceMismatch)?;
    if invoice.invoice_features().requires_unknown_bits()
        || invoice.invoice_request_features().requires_unknown_bits()
    {
        return Err(AdapterError::InvoiceMismatch);
    }
    Ok(invoice)
}

fn canonical_text(text: &str) -> Result<()> {
    if text.is_empty()
        || text.len() > MAX_INVOICE_BYTES
        || text
            .bytes()
            .any(|b| !b.is_ascii_lowercase() && !b.is_ascii_digit())
    {
        Err(AdapterError::InvoiceMismatch)
    } else {
        Ok(())
    }
}
fn milliseconds(duration: Duration) -> Result<i64> {
    i64::try_from(duration.as_millis()).map_err(|_| AdapterError::InvoiceMismatch)
}
fn decode32(text: &str) -> Result<[u8; 32]> {
    if text.len() != 64 || text.bytes().any(|b| !b.is_ascii_hexdigit()) {
        return Err(AdapterError::InvalidResponse);
    }
    hex::decode(text)
        .map_err(|_| AdapterError::InvalidResponse)?
        .try_into()
        .map_err(|_| AdapterError::InvalidResponse)
}
fn validate_invoice_values(
    amount: u64,
    hash: [u8; 32],
    created: i64,
    expires: i64,
    expected: InvoiceExpectation,
    observation: Observation,
) -> Result<()> {
    if amount == 0
        || amount > i64::MAX as u64
        || amount != expected.amount_msat
        || hash == [0; 32]
        || expected.expected_payment_hash.map_or(false, |h| h != hash)
        || created > observation.latest()?
        || expires <= observation.latest()?
    {
        Err(AdapterError::InvoiceMismatch)
    } else {
        Ok(())
    }
}
fn msat(value: Option<&Value>) -> Result<u64> {
    let value = value.ok_or(AdapterError::InvalidResponse)?;
    let amount = if let Some(n) = value.as_u64() {
        n
    } else {
        value
            .as_str()
            .and_then(|s| s.strip_suffix("msat"))
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or(AdapterError::InvalidResponse)?
    };
    if amount > i64::MAX as u64 {
        Err(AdapterError::InvalidResponse)
    } else {
        Ok(amount)
    }
}
fn seconds(value: Option<&Value>) -> Result<i64> {
    let n = value
        .and_then(Value::as_u64)
        .ok_or(AdapterError::InvalidResponse)?;
    i64::try_from(n.checked_mul(1000).ok_or(AdapterError::InvalidResponse)?)
        .map_err(|_| AdapterError::InvalidResponse)
}

#[cfg(test)]
mod tests;
