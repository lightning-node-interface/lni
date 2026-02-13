use lni::spark::lib::{SparkConfig, SparkNode as CoreSparkNode};
use lni::{CreateInvoiceParams, CreateOfferParams, LookupInvoiceParams, PayInvoiceParams};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Spark Node wrapper for napi-rs
/// Note: SparkNode requires async initialization, so we use a builder pattern
#[napi]
pub struct SparkNode {
    inner: Arc<RwLock<Option<CoreSparkNode>>>,
    config: SparkConfig,
}

#[napi]
impl SparkNode {
    #[napi(constructor)]
    pub fn new(config: SparkConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// Connect to the Spark network (must be called before using other methods)
    #[napi]
    pub async fn connect(&self) -> napi::Result<()> {
        let node = CoreSparkNode::new(self.config.clone())
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        
        let mut inner = self.inner.write().await;
        *inner = Some(node);
        Ok(())
    }

    /// Disconnect from the Spark network
    #[napi]
    pub async fn disconnect(&self) -> napi::Result<()> {
        let inner = self.inner.read().await;
        if let Some(node) = inner.as_ref() {
            node.disconnect()
                .await
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;
        }
        Ok(())
    }

    /// Check if the node is connected
    #[napi]
    pub async fn is_connected(&self) -> bool {
        let inner = self.inner.read().await;
        inner.is_some()
    }

    #[napi]
    pub fn get_mnemonic(&self) -> String {
        self.config.mnemonic.clone()
    }

    #[napi]
    pub fn get_config(&self) -> SparkConfig {
        self.config.clone()
    }

    /// Get the Spark address for receiving payments
    #[napi]
    pub async fn get_spark_address(&self) -> napi::Result<String> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.get_spark_address()
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    /// Get a Bitcoin address for on-chain deposits
    #[napi]
    pub async fn get_deposit_address(&self) -> napi::Result<String> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.get_deposit_address()
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn get_info(&self) -> napi::Result<lni::NodeInfo> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.get_info()
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> napi::Result<lni::Transaction> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.create_invoice(params)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> napi::Result<lni::types::PayInvoiceResponse> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.pay_invoice(params)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn create_offer(&self, params: CreateOfferParams) -> Result<lni::Offer> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.create_offer(params)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn get_offer(&self, search: Option<String>) -> napi::Result<lni::Offer> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.get_offer(search)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn list_offers(&self, search: Option<String>) -> napi::Result<Vec<lni::Offer>> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.list_offers(search)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> napi::Result<lni::Transaction> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.lookup_invoice(params)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> napi::Result<lni::PayInvoiceResponse> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.pay_offer(offer, amount_msats, payer_note)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn list_transactions(
        &self,
        params: crate::ListTransactionsParams,
    ) -> napi::Result<Vec<lni::Transaction>> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        let lni_params = lni::ListTransactionsParams {
            from: params.from,
            limit: params.limit,
            search: params.search,
            payment_hash: params.payment_hash,
            created_after: params.created_after,
            created_before: params.created_before,
        };
        
        node.list_transactions(lni_params)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn decode(&self, str: String) -> napi::Result<String> {
        let inner = self.inner.read().await;
        let node = inner
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("SparkNode not connected. Call connect() first.".to_string()))?;
        
        node.decode(str)
            .await
            .map_err(|e| napi::Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn on_invoice_events<T: Fn(String, Option<lni::Transaction>) -> Result<()>>(
        &self,
        params: lni::types::OnInvoiceEventParams,
        callback: T,
    ) -> Result<()> {
        let inner = self.inner.clone();

        // Block on the async function in the current thread, similar to Blink's sync approach
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let guard = inner.read().await;
            if let Some(node) = guard.as_ref() {
                let sdk = node.get_sdk();
                lni::spark::api::poll_invoice_events(sdk, params, move |status, tx| {
                    let _ = callback(status.clone(), tx.clone())
                        .map_err(|err| napi::Error::from_reason(err.to_string()));
                })
                .await;
            }
        });

        Ok(())
    }
}
