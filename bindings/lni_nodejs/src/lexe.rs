use lni::lexe::{LexeConfig, LexeNode as CoreLexeNode};
use napi::threadsafe_function::{
  ErrorStrategy, ThreadSafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode,
};
use napi::JsFunction;
use napi_derive::napi;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tokio::task::JoinHandle;

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

/// An active invoice monitor. Aborting its callback discards queued events.
struct Monitor {
  id: u32,
  task: JoinHandle<()>,
  callback: ThreadsafeFunction<String, ErrorStrategy::Fatal>,
}

impl Monitor {
  fn stop(self) {
    self.task.abort();
    // Abort rather than release, so Node drops events queued before close.
    let _ = self.callback.abort();
  }
}

#[derive(Default)]
struct State {
  node: Option<CoreLexeNode>,
  closed: bool,
  monitors: Vec<Monitor>,
  next_monitor_id: u32,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
  // State remains consistent at every await-free step, so recover after a panic.
  state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Native errors start with a stable code, `LEXE_<NAME>: message`. The
/// JavaScript entry point moves it to `error.code`; LEXE.md lists the codes.
fn coded(code: &str, message: &str) -> napi::Error {
  napi::Error::from_reason(format!("{code}: {message}"))
}

fn closed() -> napi::Error {
  coded("LEXE_NODE_CLOSED", "Lexe node closed")
}

/// Input errors keep the core's message so callers can tell a request to fix
/// from a failure to retry; Lexe Lightning payments that timed out report
/// their unknown outcome. Everything else uses `fallback`, because upstream SDK
/// errors may carry sensitive detail.
fn lexe_error(fallback: &'static str) -> impl FnOnce(lni::ApiError) -> napi::Error {
  move |error| match error {
    lni::ApiError::InvalidInput(message) => coded("LEXE_INVALID_INPUT", &message),
    lni::ApiError::Nwc { code, .. } if code == "PAYMENT_INDETERMINATE" => coded(
      "LEXE_PAYMENT_INDETERMINATE",
      "Lexe payment outcome is unknown; call lookupInvoice with the payment hash before retrying",
    ),
    _ => coded("LEXE_REQUEST_FAILED", fallback),
  }
}

/// Deregisters a monitor however its task ends, releasing the callback.
struct MonitorRegistration {
  state: Arc<Mutex<State>>,
  id: u32,
}

impl Drop for MonitorRegistration {
  fn drop(&mut self) {
    let id = self.id;
    lock(&self.state)
      .monitors
      .retain(|monitor| monitor.id != id);
  }
}

/// Lexe access for Node.js. Without a data directory, uses a stateless wallet.
#[napi]
pub struct LexeNode {
  state: Arc<Mutex<State>>,
  persistent: bool,
}

impl LexeNode {
  fn node(&self) -> napi::Result<CoreLexeNode> {
    lock(&self.state).node.clone().ok_or_else(closed)
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
      return Err(coded("LEXE_INVALID_INPUT", "dataDir must not be empty"));
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
    // Credential parse errors may echo the credential, so keep a fixed message.
    .map_err(|error| match error {
      lni::ApiError::InvalidInput(_) => coded(
        "LEXE_INVALID_INPUT",
        "Invalid Lexe client credentials, network or dataDir",
      ),
      _ => coded("LEXE_REQUEST_FAILED", "Failed to initialize Lexe node"),
    })?;
    Ok(Self {
      state: Arc::new(Mutex::new(State {
        node: Some(inner),
        ..State::default()
      })),
      persistent: data_dir.is_some(),
    })
  }

  /// Releases the wallet and stops invoice monitors without further callbacks.
  /// Later calls reject; calls already in progress may still settle. Idempotent.
  #[napi]
  pub fn close(&self) -> napi::Result<()> {
    let monitors = {
      let mut state = lock(&self.state);
      state.node = None;
      state.closed = true;
      std::mem::take(&mut state.monitors)
    };
    for monitor in monitors {
      monitor.stop();
    }
    Ok(())
  }

  #[napi]
  pub async fn get_permissions(&self) -> napi::Result<lni::Permissions> {
    self
      .node()?
      .get_permissions()
      .await
      .map_err(lexe_error("Failed to get Lexe permissions"))
  }

  #[napi]
  pub async fn get_client_info(&self) -> napi::Result<lni::lexe::LexeClientInfo> {
    self
      .node()?
      .get_client_info()
      .await
      .map_err(lexe_error("Failed to get Lexe client info"))
  }

  #[napi]
  pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    self
      .node()?
      .get_info()
      .await
      .map_err(lexe_error("Failed to get Lexe node info"))
  }

  #[napi]
  pub async fn get_receiving_capabilities(
    &self,
  ) -> napi::Result<lni::lexe::LexeReceivingCapabilities> {
    self
      .node()?
      .get_receiving_capabilities()
      .await
      .map_err(lexe_error("Failed to inspect Lexe receiving capabilities"))
  }

  #[napi]
  pub async fn get_human_bitcoin_address(
    &self,
  ) -> napi::Result<lni::lexe::LexeHumanBitcoinAddress> {
    self
      .node()?
      .get_human_bitcoin_address()
      .await
      .map_err(lexe_error("Failed to get Lexe human Bitcoin address"))
  }

  #[napi]
  pub async fn get_next_unused_address(&self) -> napi::Result<String> {
    self
      .node()?
      .get_next_unused_address()
      .await
      .map_err(lexe_error("Failed to get Lexe receiving address"))
  }

  #[napi]
  pub async fn get_exclusive_address(&self) -> napi::Result<String> {
    self
      .node()?
      .get_exclusive_address()
      .await
      .map_err(lexe_error(
        "Failed to get Lexe exclusive address; node v0.10.9 or later is required",
      ))
  }

  #[napi]
  pub async fn check_address(
    &self,
    address: String,
    synced_block_height: Option<u32>,
  ) -> napi::Result<lni::lexe::LexeAddressCheck> {
    self
      .node()?
      .check_address(address, synced_block_height)
      .await
      .map_err(lexe_error("Failed to sync Lexe exclusive address"))
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
      .map_err(lexe_error("Failed to create Lexe invoice"))
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
      .map_err(lexe_error("Failed to pay Lexe invoice"))
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
      .map_err(lexe_error("Failed to prepare Lexe on-chain transaction"))
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
      // The core rejects bad input before sending; any later error may follow
      // a completed send, so it never reads as a plain failure.
      .map_err(|error| match error {
        lni::ApiError::InvalidInput(message) => coded("LEXE_INVALID_INPUT", &message),
        _ => coded(
          "LEXE_PAYMENT_INDETERMINATE",
          lni::lexe::onchain::ONCHAIN_OUTCOME_UNKNOWN,
        ),
      })
  }

  #[napi]
  pub async fn create_offer(&self, params: lni::CreateOfferParams) -> napi::Result<lni::Offer> {
    self
      .node()?
      .create_offer(params)
      .await
      .map_err(lexe_error("Failed to create Lexe offer"))
  }

  #[napi]
  pub async fn get_offer(&self, search: Option<String>) -> napi::Result<lni::Offer> {
    self
      .node()?
      .get_offer(search)
      .await
      .map_err(lexe_error("Lexe getOffer is unavailable"))
  }

  #[napi]
  pub async fn list_offers(&self, search: Option<String>) -> napi::Result<Vec<lni::Offer>> {
    self
      .node()?
      .list_offers(search)
      .await
      .map_err(lexe_error("Lexe listOffers is unavailable"))
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
      .map_err(lexe_error("Failed to pay Lexe offer"))
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
      .map_err(lexe_error("Failed to look up Lexe invoice"))
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
      .map_err(lexe_error("Failed to list Lexe transactions"))
  }

  #[napi]
  pub async fn decode(&self, value: String) -> napi::Result<String> {
    self
      .node()?
      .decode(value)
      .await
      .map_err(lexe_error("Failed to decode Lexe payment"))
  }

  #[napi]
  pub async fn decode_offer(&self, offer: String) -> napi::Result<String> {
    self
      .node()?
      .decode_offer(offer)
      .await
      .map_err(lexe_error("Failed to decode Lexe offer"))
  }

  /// Starts native polling without blocking Node's event loop. Callback receives
  /// serialized event data, which the JavaScript entrypoint converts to LNI shape.
  /// Returns the monitor's id for `stopInvoiceEvents()`. `close()` stops every
  /// monitor; no callback is delivered after either returns.
  #[napi]
  pub fn on_invoice_events_raw(
    &self,
    params: lni::OnInvoiceEventParams,
    callback: JsFunction,
  ) -> napi::Result<u32> {
    if !self.persistent {
      return Err(coded(
        "LEXE_INVALID_INPUT",
        "Lexe invoice monitoring requires dataDir",
      ));
    }
    let inner = self.node()?;
    self.spawn_monitor(callback, move |callback| async move {
      inner.on_invoice_events(params, callback).await;
    })
  }

  /// Stops one monitor without further callbacks. Unknown or finished ids are
  /// ignored, so this is idempotent.
  #[napi]
  pub fn stop_invoice_events(&self, monitor_id: u32) {
    let monitor = {
      let mut state = lock(&self.state);
      let index = state
        .monitors
        .iter()
        .position(|monitor| monitor.id == monitor_id);
      index.map(|index| state.monitors.swap_remove(index))
    };
    if let Some(monitor) = monitor {
      monitor.stop();
    }
  }
}

impl LexeNode {
  /// Registers under the state lock, so `close()` either stops this monitor
  /// or this call observes the closed node.
  fn spawn_monitor<F, Fut>(&self, callback: JsFunction, run: F) -> napi::Result<u32>
  where
    F: FnOnce(Arc<dyn lni::OnInvoiceEventCallback>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
  {
    let mut state = lock(&self.state);
    if state.closed {
      return Err(closed());
    }
    let callback: ThreadsafeFunction<String, ErrorStrategy::Fatal> = callback
      .create_threadsafe_function(0, |ctx: ThreadSafeCallContext<String>| {
        Ok(vec![ctx.env.create_string(&ctx.value)?])
      })?;
    let id = state.next_monitor_id;
    state.next_monitor_id = state.next_monitor_id.wrapping_add(1);
    let registration = MonitorRegistration {
      state: self.state.clone(),
      id,
    };
    let events = Arc::new(InvoiceEventCallback {
      callback: callback.clone(),
    });
    // The task cannot deregister before it is recorded: that needs this lock.
    let task = tokio::spawn(async move {
      let _registration = registration;
      run(events).await;
    });
    state.monitors.push(Monitor { id, task, callback });
    Ok(id)
  }
}

/// Offline lifecycle checks for invoice monitors, built only with the
/// `lexe_monitor_test` feature. They use the same registration and close path
/// as `onInvoiceEventsRaw`, with a synthetic task and a drop-counted resource
/// in place of Lexe polling and the cloned wallet. No wallet or network is used.
#[cfg(feature = "lexe_monitor_test")]
mod monitor_test {
  use super::*;
  use std::sync::atomic::{AtomicU32, Ordering};

  static LIVE_RESOURCES: AtomicU32 = AtomicU32::new(0);

  struct Resource;

  impl Resource {
    fn new() -> Self {
      LIVE_RESOURCES.fetch_add(1, Ordering::SeqCst);
      Self
    }
  }

  impl Drop for Resource {
    fn drop(&mut self) {
      LIVE_RESOURCES.fetch_sub(1, Ordering::SeqCst);
    }
  }

  #[napi]
  impl LexeNode {
    #[napi(factory)]
    pub fn monitor_test_instance() -> Self {
      Self {
        state: Arc::new(Mutex::new(State::default())),
        persistent: true,
      }
    }

    /// Emits `pending` every `interval_ms`, then `success` after `count` events.
    #[napi]
    pub fn start_test_monitor(
      &self,
      interval_ms: u32,
      count: u32,
      callback: JsFunction,
    ) -> napi::Result<u32> {
      // Captured at registration, like the wallet clone in onInvoiceEventsRaw.
      let resource = Resource::new();
      self.spawn_monitor(callback, move |callback| async move {
        let _resource = resource;
        for _ in 0..count {
          tokio::time::sleep(std::time::Duration::from_millis(interval_ms.into())).await;
          callback.pending(None);
        }
        callback.success(None);
      })
    }

    #[napi]
    pub fn active_monitor_count(&self) -> u32 {
      lock(&self.state).monitors.len() as u32
    }

    #[napi]
    pub fn live_test_resources() -> u32 {
      LIVE_RESOURCES.load(Ordering::SeqCst)
    }
  }
}
