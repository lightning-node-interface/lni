use lni::{
  nwc::lib::NwcConfig, CreateInvoiceParams, LookupInvoiceParams, PayInvoiceParams,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

#[napi]
pub struct NwcNode {
  inner: NwcConfig,
}

#[napi]
impl NwcNode {
  #[napi(constructor)]
  pub fn new(config: NwcConfig) -> Self {
    Self { inner: config }
  }

  #[napi]
  pub fn get_nwc_uri(&self) -> String {
    self.inner.nwc_uri.clone()
  }

  #[napi]
  pub fn get_socks5_proxy(&self) -> Option<String> {
    self.inner.socks5_proxy.clone()
  }

  #[napi]
  pub fn get_config(&self) -> NwcConfig {
    self.inner.clone()
  }

  #[napi]
  pub fn get_info(&self) -> napi::Result<lni::NodeInfo> {
    let info = lni::nwc::api::get_info_sync(&self.inner)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(info)
  }

  #[napi]
  pub fn create_invoice(&self, params: CreateInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::nwc::api::create_invoice_sync(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_invoice(&self, params: PayInvoiceParams) -> Result<lni::types::PayInvoiceResponse> {
    let invoice = lni::nwc::api::pay_invoice_sync(&self.inner, params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(invoice)
  }

  #[napi]
  pub fn get_offer(&self, search: Option<String>) -> Result<lni::PayCode> {
    let paycode = lni::nwc::api::get_offer(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycode)
  }

  #[napi]
  pub fn list_offers(&self, search: Option<String>) -> Result<Vec<lni::PayCode>> {
    let paycodes = lni::nwc::api::list_offers(&self.inner, search)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(paycodes)
  }

  #[napi]
  pub fn lookup_invoice(&self, params: LookupInvoiceParams) -> napi::Result<lni::Transaction> {
    let txn = lni::nwc::api::lookup_invoice_sync(
      &self.inner,
      params.payment_hash,
      params.search,
    )
    .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txn)
  }

  #[napi]
  pub fn pay_offer(
    &self,
    offer: String,
    amount_msats: i64,
    payer_note: Option<String>,
  ) -> napi::Result<lni::PayInvoiceResponse> {
    let offer = lni::nwc::api::pay_offer(&self.inner, offer, amount_msats, payer_note)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(offer)
  }

  #[napi]
  pub fn list_transactions(
    &self,
    params: crate::ListTransactionsParams,
  ) -> napi::Result<Vec<lni::Transaction>> {
    let nwc_params = lni::ListTransactionsParams {
      from: params.from,
      limit: params.limit,
      payment_hash: params.payment_hash,
      search: params.search,
    };
    let txns = lni::nwc::api::list_transactions_sync(&self.inner, nwc_params)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(txns)
  }

  #[napi]
  pub fn decode(&self, str: String) -> Result<String> {
    let decoded = lni::nwc::api::decode(&self.inner, str)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    Ok(decoded)
  }

  // Note: This function uses sync runtime internally and spawns a background thread
  #[napi]
  pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
    &self,
    params: lni::types::OnInvoiceEventParams,
    callback: T,
  ) -> Result<()> {
    // Create a wrapper callback for the napi callback
    struct NapiCallback {
      success_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
      pending_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
      failure_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl lni::types::OnInvoiceEventCallback for NapiCallback {
      fn success(&self, _transaction: Option<lni::Transaction>) {
        self.success_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Note: Cannot call napi callback directly from different thread
      }
      
      fn pending(&self, _transaction: Option<lni::Transaction>) {
        self.pending_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      }
      
      fn failure(&self, _transaction: Option<lni::Transaction>) {
        self.failure_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      }
    }

    let napi_callback = NapiCallback {
      success_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
      pending_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
      failure_count: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };

    lni::nwc::api::on_invoice_events(self.inner.clone(), params, Box::new(napi_callback));
    Ok(())
  }

  #[napi]
  pub fn on_invoice_events_cancel(
    &self,
    params: lni::types::OnInvoiceEventParams,
  ) -> Result<InvoiceEventsHandle> {
    // Create channels to communicate results
    let (tx, rx) = std::sync::mpsc::channel::<(String, Option<lni::Transaction>)>();

    // Create the callback that sends to the channel
    struct ChannelCallback {
      sender: std::sync::mpsc::Sender<(String, Option<lni::Transaction>)>,
    }

    impl lni::types::OnInvoiceEventCallback for ChannelCallback {
      fn success(&self, transaction: Option<lni::Transaction>) {
        let _ = self.sender.send(("success".to_string(), transaction));
      }
      
      fn pending(&self, transaction: Option<lni::Transaction>) {
        let _ = self.sender.send(("pending".to_string(), transaction));
      }
      
      fn failure(&self, transaction: Option<lni::Transaction>) {
        let _ = self.sender.send(("failure".to_string(), transaction));
      }
    }

    let callback = ChannelCallback { sender: tx };
    let config = self.inner.clone();

    // Use the existing cancellation-aware function
    let cancellation = lni::nwc::api::on_invoice_events_with_cancellation(
      config,
      params,
      Box::new(callback),
    );

    Ok(InvoiceEventsHandle { 
      cancellation: InvoiceEventsCancellation { inner: cancellation },
      receiver: std::sync::Arc::new(std::sync::Mutex::new(rx)),
    })
  }
}

// NAPI wrapper for handling events with cancellation
#[napi]
pub struct InvoiceEventsHandle {
  cancellation: InvoiceEventsCancellation,
  receiver: std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<(String, Option<lni::Transaction>)>>>,
}

#[napi]
impl InvoiceEventsHandle {
  #[napi]
  pub fn cancel(&self) {
    self.cancellation.cancel();
  }

  #[napi]
  pub fn is_cancelled(&self) -> bool {
    self.cancellation.is_cancelled()
  }

  #[napi]
  pub fn poll_event(&self) -> Option<InvoiceEvent> {
    if let Ok(receiver) = self.receiver.lock() {
      if let Ok((status, transaction)) = receiver.try_recv() {
        return Some(InvoiceEvent { status, transaction });
      }
    }
    None
  }

  #[napi]
  pub fn wait_for_event(&self, timeout_ms: u32) -> Option<InvoiceEvent> {
    if let Ok(receiver) = self.receiver.lock() {
      let timeout = std::time::Duration::from_millis(timeout_ms as u64);
      if let Ok((status, transaction)) = receiver.recv_timeout(timeout) {
        return Some(InvoiceEvent { status, transaction });
      }
    }
    None
  }
}

#[napi(object)]
pub struct InvoiceEvent {
  pub status: String,
  pub transaction: Option<lni::Transaction>,
}

// NAPI wrapper for the cancellation token
#[napi]
pub struct InvoiceEventsCancellation {
  inner: std::sync::Arc<lni::nwc::api::InvoiceEventsCancellation>,
}

#[napi]
impl InvoiceEventsCancellation {
  #[napi]
  pub fn cancel(&self) {
    self.inner.cancel();
  }

  #[napi]
  pub fn is_cancelled(&self) -> bool {
    self.inner.is_cancelled()
  }
}
