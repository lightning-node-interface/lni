//! Explicit native foundation payer. Ordinary Phoenixd pay-offer is never used.
//! Requires the separately reviewed local wallet capability profile. Requests
//! never retry; lost dispatch replies remain uncertain and recovery is read-only.
use super::PhoenixdConfig;
use crate::cln::foundation::{
    AdapterError, Chain, ConstructionPaymentProof, DispatchRequest, InvoiceExpectation,
    InvoiceVerifier, NativeAccount, NativeSettlement, Observation, PaymentStatus, ValidatedInvoice,
};
use reqwest::{Method, StatusCode, Url};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    net::IpAddr,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, AdapterError>;
const MAX_RESPONSE: usize = 65536;

pub struct PhoenixdFoundation {
    base: Url,
    client: reqwest::Client,
    password: String,
    account: NativeAccount,
    verifier: InvoiceVerifier,
}
impl PhoenixdFoundation {
    pub async fn connect(config: PhoenixdConfig, account: NativeAccount) -> Result<Self> {
        let base = Url::parse(&config.url).map_err(|_| AdapterError::InvalidInput)?;
        let ip = base
            .host_str()
            .and_then(|v| v.trim_matches(['[', ']']).parse::<IpAddr>().ok());
        if !ip.map_or(false, |ip| ip.is_loopback())
            || !matches!(base.scheme(), "http" | "https")
            || base.path() != "/"
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
            || config.password.is_empty()
            || config.password.len() > 4096
            || config.accept_invalid_certs == Some(true)
            || config
                .socks5_proxy
                .as_deref()
                .map_or(false, |v| !v.is_empty())
        {
            return Err(AdapterError::InvalidInput);
        }
        let timeout = config.http_timeout.unwrap_or(60);
        if timeout <= 0 || timeout > 120 {
            return Err(AdapterError::InvalidInput);
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(timeout as u64))
            .build()
            .map_err(|_| AdapterError::InvalidInput)?;
        let wallet = Self {
            base,
            client,
            password: config.password,
            account,
            verifier: InvoiceVerifier::new(account)?,
        };
        wallet.check_account().await?;
        let caps = wallet
            .http(Method::GET, "eltor/capabilities", None)
            .await?
            .ok_or(AdapterError::Unsupported)?;
        if caps.get("version").and_then(Value::as_u64) != Some(1)
            || caps.get("fetchBeforePay").and_then(Value::as_u64) != Some(1)
            || caps.get("boundedFeeMsat").and_then(Value::as_u64) != Some(1)
        {
            return Err(AdapterError::Unsupported);
        }
        Ok(wallet)
    }
    pub fn account(&self) -> NativeAccount {
        self.account
    }
    pub async fn check_account(&self) -> Result<()> {
        let info = self
            .http(Method::GET, "getinfo", None)
            .await?
            .ok_or(AdapterError::InvalidResponse)?;
        let chain = match info.get("chain").and_then(Value::as_str) {
            Some("mainnet") | Some("bitcoin") => Chain::Bitcoin,
            Some("testnet") | Some("testnet3") => Chain::Testnet,
            Some("regtest") => Chain::Regtest,
            Some("signet") => Chain::Signet,
            _ => return Err(AdapterError::Unsupported),
        };
        if info.get("nodeId").and_then(Value::as_str)
            != Some(hex::encode(self.account.node_id).as_str())
            || chain != self.account.chain
        {
            return Err(AdapterError::AccountMismatch);
        }
        Ok(())
    }
    pub async fn fetch_bolt12(
        &self,
        offer: &str,
        payment_id: [u8; 32],
        expected: InvoiceExpectation,
        observation: Observation,
    ) -> Result<ValidatedInvoice> {
        let started = Instant::now();
        self.verifier.validate_bolt12_offer(offer)?;
        if payment_id == [0; 32]
            || expected.amount_msat == 0
            || expected.amount_msat > i64::MAX as u64
        {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        let fields = vec![
            ("offer", offer.to_owned()),
            ("amountMsat", expected.amount_msat.to_string()),
            ("payerNote", hex::encode(payment_id)),
        ];
        let reply = self
            .http(Method::POST, "eltor/fetchinvoice", Some(fields))
            .await?
            .ok_or(AdapterError::InvalidResponse)?;
        let invoice = reply
            .get("invoice")
            .and_then(Value::as_str)
            .ok_or(AdapterError::InvalidResponse)?;
        self.verifier.inspect_bolt12_for_offer(
            invoice,
            offer,
            payment_id,
            expected,
            advance(observation, started.elapsed())?,
        )
    }
    pub fn restore_for_lookup(
        &self,
        text: &str,
        digest: [u8; 32],
        expected: InvoiceExpectation,
    ) -> Result<ValidatedInvoice> {
        self.verifier.restore_for_lookup(text, digest, expected)
    }
    pub fn restore_bolt12_for_offer_lookup(
        &self,
        text: &str,
        digest: [u8; 32],
        offer: &str,
        payment_id: [u8; 32],
        expected: InvoiceExpectation,
    ) -> Result<ValidatedInvoice> {
        self.verifier
            .restore_bolt12_for_offer_lookup(text, digest, offer, payment_id, expected)
    }
    pub async fn dispatch(
        &self,
        invoice: &ValidatedInvoice,
        request: DispatchRequest,
        observation: Observation,
    ) -> Result<PaymentStatus> {
        let started = Instant::now();
        let now = latest(observation)?;
        if invoice.account() != self.account
            || !invoice.invoice().starts_with("lni1")
            || request.operation == [0; 16]
            || request.fee_limit_msat % 1000 != 0
            || request.fee_limit_msat > i64::MAX as u64
            || invoice
                .amount_msat()
                .checked_add(request.fee_limit_msat)
                .map_or(true, |n| n > i64::MAX as u64)
            || request.retry_for_seconds == 0
            || request.pay_before_ms > invoice.expires_at_ms()
            || request.pay_before_ms <= now
            || i64::from(request.retry_for_seconds) * 1000 > request.pay_before_ms - now
        {
            return Err(AdapterError::InvalidInput);
        }
        self.check_account().await?;
        if let Some(row) = self.record(request.operation).await? {
            return self
                .interpret(
                    invoice,
                    request.operation,
                    &row,
                    advance(observation, started.elapsed())?,
                )
                .map(|v| v.0);
        }
        // Account/lookup latency consumes the original dispatch window; it cannot restart it.
        if request
            .pay_before_ms
            .saturating_sub(latest(advance(observation, started.elapsed())?)?)
            < i64::from(request.retry_for_seconds) * 1000
        {
            return Err(AdapterError::InvalidInput);
        }
        let fields = vec![
            ("operation", operation_text(request.operation)),
            ("invoice", invoice.invoice().to_owned()),
            ("amountMsat", invoice.amount_msat().to_string()),
            ("maxFeeMsat", request.fee_limit_msat.to_string()),
        ];
        self.http(Method::POST, "eltor/payinvoice", Some(fields))
            .await?;
        self.lookup(
            invoice,
            request.operation,
            advance(observation, started.elapsed())?,
        )
        .await
    }
    pub async fn lookup(
        &self,
        invoice: &ValidatedInvoice,
        operation: [u8; 16],
        observation: Observation,
    ) -> Result<PaymentStatus> {
        let started = Instant::now();
        self.bound(invoice, operation)?;
        self.check_account().await?;
        match self.record(operation).await? {
            Some(row) => self
                .interpret(
                    invoice,
                    operation,
                    &row,
                    advance(observation, started.elapsed())?,
                )
                .map(|v| v.0),
            None => Ok(PaymentStatus::Unknown),
        }
    }
    pub async fn lookup_construction_proof(
        &self,
        invoice: &ValidatedInvoice,
        operation: [u8; 16],
        observation: Observation,
    ) -> Result<Option<ConstructionPaymentProof>> {
        let started = Instant::now();
        self.bound(invoice, operation)?;
        self.check_account().await?;
        let Some(row) = self.record(operation).await? else {
            return Ok(None);
        };
        match self.interpret(
            invoice,
            operation,
            &row,
            advance(observation, started.elapsed())?,
        )? {
            (PaymentStatus::Settled(settlement), Some(proof)) => {
                ConstructionPaymentProof::from_verified_native(settlement, proof).map(Some)
            }
            _ => Ok(None),
        }
    }
    fn bound(&self, invoice: &ValidatedInvoice, operation: [u8; 16]) -> Result<()> {
        if invoice.account() != self.account || operation == [0; 16] {
            return Err(AdapterError::InvalidInput);
        }
        Ok(())
    }
    async fn record(&self, operation: [u8; 16]) -> Result<Option<Value>> {
        self.http(
            Method::GET,
            &format!("eltor/outgoing/{}", operation_text(operation)),
            None,
        )
        .await
    }
    fn interpret(
        &self,
        invoice: &ValidatedInvoice,
        operation: [u8; 16],
        row: &Value,
        observation: Observation,
    ) -> Result<(PaymentStatus, Option<[u8; 32]>)> {
        self.bound(invoice, operation)?;
        if row.get("operation").and_then(Value::as_str) != Some(operation_text(operation).as_str())
            || row.get("invoice").and_then(Value::as_str) != Some(invoice.invoice())
            || row.get("paymentHash").and_then(Value::as_str)
                != Some(hex::encode(invoice.payment_hash()).as_str())
            || row.get("amountMsat").and_then(Value::as_u64) != Some(invoice.amount_msat())
        {
            return Err(AdapterError::InvoiceMismatch);
        }
        match row.get("state").and_then(Value::as_str) {
            Some("pending") | Some("failed") => {
                if row.get("preimage").map_or(false, |v| !v.is_null()) {
                    return Err(AdapterError::InvalidResponse);
                }
                if row.get("state").and_then(Value::as_str) == Some("pending") {
                    if row.get("completedAt").map_or(false, |v| !v.is_null()) {
                        return Err(AdapterError::InvalidResponse);
                    }
                    Ok((PaymentStatus::Pending, None))
                } else {
                    let created = row
                        .get("createdAt")
                        .and_then(Value::as_i64)
                        .ok_or(AdapterError::InvalidResponse)?;
                    let completed = row
                        .get("completedAt")
                        .and_then(Value::as_i64)
                        .ok_or(AdapterError::InvalidResponse)?;
                    if row.get("feeMsat").and_then(Value::as_u64) != Some(0)
                        || created < 0
                        || completed < created
                        || completed > latest(observation)?
                    {
                        return Err(AdapterError::InvalidResponse);
                    }
                    Ok((PaymentStatus::FailedFinal, None))
                }
            }
            Some("paid") => {
                let proof = decode32(
                    row.get("preimage")
                        .and_then(Value::as_str)
                        .ok_or(AdapterError::InvalidResponse)?,
                )?;
                let fee = row
                    .get("feeMsat")
                    .and_then(Value::as_u64)
                    .ok_or(AdapterError::InvalidResponse)?;
                let created = row
                    .get("createdAt")
                    .and_then(Value::as_i64)
                    .ok_or(AdapterError::InvalidResponse)?;
                let completed = row
                    .get("completedAt")
                    .and_then(Value::as_i64)
                    .ok_or(AdapterError::InvalidResponse)?;
                if created < 0
                    || completed < created
                    || completed > latest(observation)?
                    || fee > i64::MAX as u64
                    || <[u8; 32]>::from(Sha256::digest(proof)) != invoice.payment_hash()
                {
                    return Err(AdapterError::InvalidResponse);
                }
                Ok((
                    PaymentStatus::Settled(NativeSettlement {
                        account: self.account,
                        operation,
                        invoice_digest: invoice.digest(),
                        payment_hash: invoice.payment_hash(),
                        amount_msat: invoice.amount_msat(),
                        fee_msat: fee,
                        settled_earliest_ms: completed,
                        settled_latest_ms: completed,
                    }),
                    Some(proof),
                ))
            }
            _ => Ok((PaymentStatus::Unknown, None)),
        }
    }
    async fn http(
        &self,
        method: Method,
        path: &str,
        fields: Option<Vec<(&str, String)>>,
    ) -> Result<Option<Value>> {
        // StatusPages maps an absent operation to this exact 404 body. Other
        // 404s remain uncertain; this never licenses retrying a claimed pay.
        let absent_lookup = method == Method::GET && path.starts_with("eltor/outgoing/");
        let url = self
            .base
            .join(path)
            .map_err(|_| AdapterError::InvalidInput)?;
        let mut request = self
            .client
            .request(method, url)
            .basic_auth("", Some(&self.password));
        if let Some(fields) = fields {
            request = request.form(&fields);
        }
        let mut response = request.send().await.map_err(|_| AdapterError::Uncertain)?;
        if absent_lookup && response.status() == StatusCode::NOT_FOUND {
            let mut body = Vec::new();
            while let Some(part) = response
                .chunk()
                .await
                .map_err(|_| AdapterError::Uncertain)?
            {
                if part.len() > 64usize.saturating_sub(body.len()) {
                    return Err(AdapterError::Uncertain);
                }
                body.extend_from_slice(&part);
            }
            return if body == b"Not found" {
                Ok(None)
            } else {
                Err(AdapterError::Uncertain)
            };
        }
        if response.status() == StatusCode::NO_CONTENT {
            return Ok(None);
        }
        if !response.status().is_success()
            || response
                .content_length()
                .map_or(false, |n| n > MAX_RESPONSE as u64)
        {
            return Err(AdapterError::Uncertain);
        }
        let mut bytes = Vec::new();
        while let Some(part) = response
            .chunk()
            .await
            .map_err(|_| AdapterError::Uncertain)?
        {
            if part.len() > MAX_RESPONSE.saturating_sub(bytes.len()) {
                return Err(AdapterError::InvalidResponse);
            }
            bytes.extend_from_slice(&part);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| AdapterError::InvalidResponse)?;
        if !value.is_object() || value.get("error").is_some() {
            return Err(AdapterError::Uncertain);
        }
        Ok(Some(value))
    }
}
// Advance the caller's trusted UTC bracket only by observed monotonic elapsed
// time. This is an observation bound, never a new payment/circuit deadline.
fn advance(observation: Observation, elapsed: Duration) -> Result<Observation> {
    let elapsed = i64::try_from(elapsed.as_millis()).map_err(|_| AdapterError::InvalidInput)?;
    Ok(Observation {
        now_ms: observation
            .now_ms
            .checked_add(elapsed)
            .ok_or(AdapterError::InvalidInput)?,
        uncertainty_ms: observation.uncertainty_ms,
    })
}
fn operation_text(operation: [u8; 16]) -> String {
    uuid::Uuid::from_bytes(operation).hyphenated().to_string()
}
fn latest(observation: Observation) -> Result<i64> {
    if observation.now_ms < 0 {
        return Err(AdapterError::InvalidInput);
    }
    observation
        .now_ms
        .checked_add(
            i64::try_from(observation.uncertainty_ms).map_err(|_| AdapterError::InvalidInput)?,
        )
        .ok_or(AdapterError::InvalidInput)
}
fn decode32(text: &str) -> Result<[u8; 32]> {
    if text.len() != 64 {
        return Err(AdapterError::InvalidResponse);
    }
    hex::decode(text)
        .map_err(|_| AdapterError::InvalidResponse)?
        .try_into()
        .map_err(|_| AdapterError::InvalidResponse)
}
#[cfg(test)]
mod tests;
