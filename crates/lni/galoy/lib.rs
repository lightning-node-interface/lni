use std::collections::HashMap;

use crate::types::NodeInfo;
#[cfg(not(feature = "uniffi"))]
use crate::LightningNode;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams, LookupInvoiceParams,
    Offer, OnchainTransaction, PayInvoiceParams, PayInvoiceResponse, PayOnchainOptions,
    PayOnchainResponse, Permissions, PrepareOnchainTransactionParams, Transaction,
};

#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GaloyWalletConfig {
    Explicit { id: String, currency: String },
    Currency { currency: String },
}

impl GaloyWalletConfig {
    pub fn currency(&self) -> &str {
        match self {
            Self::Explicit { currency, .. } | Self::Currency { currency } => currency,
        }
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaloyPaymentResponse {
    TransactionWithPreimage,
    StatusOnly,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaloyPaymentConfig {
    pub response: GaloyPaymentResponse,
    pub accepted_statuses: Vec<String>,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaloyCapabilities {
    pub transaction_lookup: bool,
    pub transaction_history: bool,
    pub invoice_events: bool,
    pub onchain: bool,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaloyPermissionsMode {
    JwtIntrospection,
    Configured,
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct GaloyProvider {
    pub id: String,
    pub name: String,
}

impl std::fmt::Debug for GaloyProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GaloyProvider")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct GaloyConfig {
    pub api_key: String,
    pub base_url: String,
    pub provider: GaloyProvider,
    pub wallet: GaloyWalletConfig,
    pub payment: GaloyPaymentConfig,
    pub capabilities: GaloyCapabilities,
    pub permissions: GaloyPermissionsMode,
    pub additional_headers: Option<HashMap<String, String>>,
    pub http_timeout: Option<i64>,
    pub socks5_proxy: Option<String>,
    pub accept_invalid_certs: Option<bool>,
}

impl std::fmt::Debug for GaloyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let additional_header_names = self
            .additional_headers
            .as_ref()
            .map(|headers| headers.keys().cloned().collect::<Vec<_>>());
        f.debug_struct("GaloyConfig")
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("provider", &self.provider)
            .field("wallet", &self.wallet)
            .field("payment", &self.payment)
            .field("capabilities", &self.capabilities)
            .field("permissions", &self.permissions)
            .field("additional_header_names", &additional_header_names)
            .field("http_timeout", &self.http_timeout)
            .field("socks5_proxy", &"<redacted>")
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .finish()
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Debug, Clone)]
pub struct GaloyNode {
    pub config: GaloyConfig,
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl GaloyNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: GaloyConfig) -> Self {
        Self { config }
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl GaloyNode {
    pub async fn get_permissions(&self) -> Result<Permissions, ApiError> {
        match self.config.permissions {
            GaloyPermissionsMode::Configured => Ok(Permissions {
                get_info: true,
                create_invoice: true,
                pay_invoice: true,
                create_offer: false,
                get_offer: false,
                list_offers: false,
                pay_offer: false,
                lookup_invoice: self.config.capabilities.transaction_lookup,
                list_transactions: self.config.capabilities.transaction_history,
                decode: true,
                on_invoice_events: self.config.capabilities.invoice_events,
            }),
            GaloyPermissionsMode::JwtIntrospection => {
                let mut permissions =
                    crate::permissions::parse_galoy_token_permissions(&self.config.api_key)
                        .ok_or_else(|| {
                            ApiError::InvalidInput(format!(
                                "{} API keys cannot be introspected. Use a JWT-style token or configured permissions.",
                                self.config.provider.name
                            ))
                        })?;
                permissions.lookup_invoice &= self.config.capabilities.transaction_lookup;
                permissions.list_transactions &= self.config.capabilities.transaction_history;
                permissions.on_invoice_events &= self.config.capabilities.invoice_events;
                Ok(permissions)
            }
        }
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        crate::galoy::api::get_info(&self.config).await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::galoy::api::create_invoice(&self.config, params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::galoy::api::pay_invoice(&self.config, params).await
    }

    pub async fn prepare_onchain_transaction(
        &self,
        params: PrepareOnchainTransactionParams,
    ) -> Result<OnchainTransaction, ApiError> {
        crate::galoy::api::prepare_onchain_transaction(&self.config, params).await
    }

    pub async fn pay_onchain(
        &self,
        transaction: OnchainTransaction,
    ) -> Result<PayOnchainResponse, ApiError> {
        crate::galoy::api::pay_onchain(&self.config, transaction).await
    }

    pub async fn pay_onchain_with_options(
        &self,
        transaction: OnchainTransaction,
        options: PayOnchainOptions,
    ) -> Result<PayOnchainResponse, ApiError> {
        crate::galoy::api::pay_onchain_with_options(&self.config, transaction, options).await
    }

    pub async fn create_offer(&self, _params: CreateOfferParams) -> Result<Offer, ApiError> {
        crate::galoy::api::not_implemented(&self.config, "Bolt12", "make_invoice")
    }

    pub async fn get_offer(&self, _search: Option<String>) -> Result<Offer, ApiError> {
        crate::galoy::api::not_implemented(&self.config, "Bolt12", "lookup_invoice")
    }

    pub async fn list_offers(&self, _search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        crate::galoy::api::not_implemented(&self.config, "Bolt12", "list_transactions")
    }

    pub async fn pay_offer(
        &self,
        _offer: String,
        _amount_msats: i64,
        _payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        crate::galoy::api::not_implemented(&self.config, "Bolt12", "pay_invoice")
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        crate::galoy::api::lookup_invoice(
            &self.config,
            params.payment_hash,
            None,
            None,
            params.search,
        )
        .await
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, ApiError> {
        crate::galoy::api::list_transactions(&self.config, params.from, params.limit, params.search)
            .await
    }

    pub async fn decode(&self, value: String) -> Result<String, ApiError> {
        crate::utils::decode_bolt11(value)
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, ApiError> {
        crate::utils::decode_offer(offer)
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        crate::galoy::api::on_invoice_events(self.config.clone(), params, callback).await
    }
}

crate::impl_lightning_node!(GaloyNode);
