use lni::lexe::{LexeConfig, LexeNode as CoreLexeNode};
use napi::threadsafe_function::{
  ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::JsFunction;
use napi_derive::napi;
use std::sync::{Arc, Mutex};

struct InvoiceEventCallback {
  callback: ThreadsafeFunction<String, ErrorStrategy::Fatal>,
}

impl InvoiceEventCallback {
  fn send(&self, status: &str, transaction: Option<lni::Transaction>) {
    let event = serde_json::json!({ "status": status, "transaction": transaction });
    if let Ok(payload) = serde_json::to_string(&event) {
      self
        .callback
        .call(payload, ThreadsafeFunctionCallMode::NonBlocking);
    }
  }
}

impl lni::OnInvoiceEventCallback for InvoiceEventCallback {
  fn success(&self, transaction: Option<lni::Transaction>) {
    self.send("success", transaction);
  }

  fn pending(&self, transaction: Option<lni::Transaction>) {
    self.send("pending", transaction);
  }

  fn failure(&self, transaction: Option<lni::Transaction>) {
    self.send("failure", transaction);
  }
}

/// Lexe access for Node.js. Without a data directory, uses a stateless wallet.
#[napi]
pub struct LexeNode {
  inner: Mutex<Option<CoreLexeNode>>,
  persistent: bool,
}

impl LexeNode {
  fn node(&self) -> napi::Result<CoreLexeNode> {
    self
      .inner
      .lock()
      .map_err(|_| napi::Error::from_reason("Lexe node unavailable"))?
      .clone()
      .ok_or_else(|| napi::Error::from_reason("Lexe node closed"))
  }
}

#[napi]
impl LexeNode {
  #[napi(constructor)]
  pub fn new(
    client_credentials: String,
    network: Option<String>,
    data_dir: Option<String>,
  ) -> napi::Result<Self> {
    if data_dir.as_deref() == Some("") {
      return Err(napi::Error::from_reason("dataDir must not be empty"));
    }
    let config = LexeConfig {
      client_credentials,
      network,
      data_dir: data_dir.clone(),
    };
    let inner = if data_dir.is_some() {
      CoreLexeNode::new(config)
    } else {
      CoreLexeNode::new_stateless(config)
    }
    .map_err(|_| napi::Error::from_reason("Failed to initialize Lexe node"))?;
    Ok(Self {
      inner: Mutex::new(Some(inner)),
      persistent: data_dir.is_some(),
    })
  }

  #[napi]
  pub fn close(&self) -> napi::Result<()> {
    self
      .inner
      .lock()
      .map_err(|_| napi::Error::from_reason("Lexe node unavailable"))?
      .take();
    Ok(())
  }

  #[napi]
  pub async fn get_permissions(&self) -> napi::Result<lni::Permissions> {
    self
      .node()?
      .get_permissions()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe permissions"))
  }

  #[napi]
  pub async fn get_client_info(&self) -> napi::Result<lni::lexe::LexeClientInfo> {
    self
      .node()?
      .get_client_info()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe client info"))
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    self
      .node()?
      .get_info()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe node info"))
  }

  #[napi]
  pub async fn get_human_bitcoin_address(
    &self,
  ) -> napi::Result<lni::lexe::LexeHumanBitcoinAddress> {
    self
      .node()?
      .get_human_bitcoin_address()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe human Bitcoin address"))
  }

  #[napi]
  pub async fn get_next_unused_address(&self) -> napi::Result<String> {
    self
      .node()?
      .get_next_unused_address()
      .await
      .map_err(|_| napi::Error::from_reason("Failed to get Lexe receiving address"))
  }

  #[napi]
  pub async fn create_invoice(
    &self,
    params: lni::CreateInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    self
      .node()?
      .create_invoice(params)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to create Lexe invoice"))
  }

  #[napi]
  pub async fn pay_invoice(
    &self,
    params: lni::PayInvoiceParams,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    self
      .node()?
      .pay_invoice(params)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to pay Lexe invoice"))
  }

  #[napi]
  pub async fn prepare_onchain_transaction(
    &self,
    params: lni::PrepareOnchainTransactionParams,
  ) -> napi::Result<lni::OnchainTransaction> {
    self
      .node()?
      .prepare_onchain_transaction(params)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to prepare Lexe on-chain transaction"))
  }

  #[napi]
  pub async fn pay_onchain(
    &self,
    transaction: lni::OnchainTransaction,
    options: Option<lni::PayOnchainOptions>,
  ) -> napi::Result<lni::PayOnchainResponse> {
    self
      .node()?
      .pay_onchain_with_options(transaction, options.unwrap_or_default())
      .await
      .map_err(|_| napi::Error::from_reason("Failed to pay Lexe on-chain transaction"))
  }

  #[napi]
  pub async fn create_offer(&self, params: lni::CreateOfferParams) -> napi::Result<lni::Offer> {
    self
      .node()?
      .create_offer(params)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to create Lexe offer"))
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> napi::Result<lni::Offer> {
    self
      .node()?
      .get_offer(search)
      .await
      .map_err(|_| napi::Error::from_reason("Lexe getOffer is unavailable"))
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> napi::Result<Vec<lni::Offer>> {
    self
      .node()?
      .list_offers(search)
      .await
      .map_err(|_| napi::Error::from_reason("Lexe listOffers is unavailable"))
  }

  #[napi]
  pub async fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    self
      .node()?
      .pay_offer(offer, amount_msats, payer_note)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to pay Lexe offer"))
  }

  #[napi]
  pub async fn lookup_invoice(
    &self,
    params: lni::LookupInvoiceParams,
  ) -> napi::Result<lni::Transaction> {
    self
      .node()?
      .lookup_invoice(params)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to look up Lexe invoice"))
  }

  #[napi]
  pub async fn list_transactions(
    &self,
    params: lni::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    self
      .node()?
      .list_transactions(params)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to list Lexe transactions"))
  }

  #[napi]
  pub async fn decode(&self, value: String) -> napi::Result<String> {
    self
      .node()?
      .decode(value)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to decode Lexe payment"))
  }

  #[napi]
  pub async fn decode_offer(&self, offer: String) -> napi::Result<String> {
    self
      .node()?
      .decode_offer(offer)
      .await
      .map_err(|_| napi::Error::from_reason("Failed to decode Lexe offer"))
  }

  /// Starts native polling without blocking Node's event loop. Callback receives
  /// serialized event data, which the JavaScript entrypoint converts to LNI shape.
  #[napi]
  pub fn on_invoice_events_raw(
    &self,
    params: lni::OnInvoiceEventParams,
    callback: JsFunction,
  ) -> napi::Result<()> {
    if !self.persistent {
      return Err(napi::Error::from_reason(
        "Lexe invoice monitoring requires dataDir",
      ));
    }
    let callback: ThreadsafeFunction<String, ErrorStrategy::Fatal> = callback
      .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<String>| {
        Ok(vec![ctx.env.create_string(&ctx.value)?])
      })?;
    let inner = self.node()?;
    tokio::spawn(async move {
      inner
        .on_invoice_events(params, Arc::new(InvoiceEventCallback { callback }))
        .await;
    });
    Ok(())
  }
}
