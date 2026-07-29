use std::collections::HashMap;

#[cfg(not(feature = "uniffi"))]
use crate::LightningNode;
use crate::{
    ApiError, CreateInvoiceParams, CreateOfferParams, ListTransactionsParams, LookupInvoiceParams,
    NodeInfo, Offer, PayInvoiceParams, PayInvoiceResponse, Permissions, Transaction,
};

pub const DEFAULT_FLASH_GRAPHQL_URL: &str = "https://api.flashapp.me/graphql";

fn flash_fee_probe_operation(wallet_currency: &str) -> crate::galoy::GaloyInvoiceOperation {
    if wallet_currency.eq_ignore_ascii_case("BTC") {
        crate::galoy::GaloyInvoiceOperation::BtcSats
    } else if wallet_currency.eq_ignore_ascii_case("USD") {
        crate::galoy::GaloyInvoiceOperation::UsdCents
    } else {
        crate::galoy::GaloyInvoiceOperation::Unsupported
    }
}

#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[derive(Clone)]
pub struct FlashConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub wallet_id: String,
    pub wallet_currency: String,
    pub additional_headers: Option<HashMap<String, String>>,
    pub accepted_statuses: Option<Vec<String>>,
    pub http_timeout: Option<i64>,
    pub socks5_proxy: Option<String>,
    pub accept_invalid_certs: Option<bool>,
}

impl std::fmt::Debug for FlashConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let additional_header_names = self
            .additional_headers
            .as_ref()
            .map(|headers| headers.keys().cloned().collect::<Vec<_>>());
        f.debug_struct("FlashConfig")
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("wallet_id", &self.wallet_id)
            .field("wallet_currency", &self.wallet_currency)
            .field("additional_header_names", &additional_header_names)
            .field("accepted_statuses", &self.accepted_statuses)
            .field("http_timeout", &self.http_timeout)
            .field("socks5_proxy", &"<redacted>")
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .finish()
    }
}

impl From<&FlashConfig> for crate::galoy::GaloyConfig {
    fn from(config: &FlashConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            base_url: config
                .base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_FLASH_GRAPHQL_URL.to_string()),
            provider: crate::galoy::GaloyProvider {
                id: "flash".to_string(),
                name: "Flash".to_string(),
            },
            wallet: crate::galoy::GaloyWalletConfig::Explicit {
                id: config.wallet_id.clone(),
                currency: config.wallet_currency.clone(),
            },
            invoice_operations: crate::galoy::GaloyInvoiceOperationsConfig {
                create: crate::galoy::GaloyInvoiceOperation::Unsupported,
                fee_probe: flash_fee_probe_operation(&config.wallet_currency),
            },
            payment: crate::galoy::GaloyPaymentConfig {
                response: crate::galoy::GaloyPaymentResponse::StatusOnly,
                accepted_statuses: config.accepted_statuses.clone().unwrap_or_else(|| {
                    vec![
                        "SUCCESS".to_string(),
                        "PENDING".to_string(),
                        "ALREADY_PAID".to_string(),
                    ]
                }),
                status_mapping: Some(crate::galoy::GaloyPaymentStatusMapping {
                    settled: vec!["SUCCESS".to_string(), "ALREADY_PAID".to_string()],
                    pending: vec!["PENDING".to_string()],
                }),
                proof_unavailable_error_codes: vec!["PROOF_UNAVAILABLE".to_string()],
            },
            capabilities: crate::galoy::GaloyCapabilities {
                transaction_lookup: false,
                transaction_history: false,
                invoice_events: false,
                onchain: false,
            },
            permissions: crate::galoy::GaloyPermissionsMode::Configured,
            additional_headers: config.additional_headers.clone(),
            http_timeout: config.http_timeout,
            socks5_proxy: config.socks5_proxy.clone(),
            accept_invalid_certs: config.accept_invalid_certs,
        }
    }
}

/// Flash adapter backed by the generic Galoy GraphQL implementation.
///
/// Status-only payments may return an empty preimage. Use
/// [`FlashNode::pay_invoice_with_status`] when the caller must distinguish a
/// resolved `PENDING` payment from settlement.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
#[derive(Debug, Clone)]
pub struct FlashNode {
    pub config: FlashConfig,
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl FlashNode {
    #[cfg_attr(feature = "uniffi", uniffi::constructor)]
    pub fn new(config: FlashConfig) -> Self {
        Self { config }
    }

    fn galoy(&self) -> crate::galoy::GaloyNode {
        crate::galoy::GaloyNode::new((&self.config).into())
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export(async_runtime = "tokio"))]
impl FlashNode {
    pub async fn get_permissions(&self) -> Result<Permissions, ApiError> {
        self.galoy().get_permissions().await
    }

    pub async fn get_info(&self) -> Result<NodeInfo, ApiError> {
        self.galoy().get_info().await
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        self.galoy().create_invoice(params).await
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, ApiError> {
        self.galoy().pay_invoice(params).await
    }

    /// Pay an invoice while retaining Flash's accepted provider status.
    pub async fn pay_invoice_with_status(
        &self,
        params: PayInvoiceParams,
    ) -> Result<crate::galoy::GaloyPaymentOutcome, ApiError> {
        self.galoy().pay_invoice_with_status(params).await
    }

    pub async fn create_offer(&self, params: CreateOfferParams) -> Result<Offer, ApiError> {
        self.galoy().create_offer(params).await
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, ApiError> {
        self.galoy().get_offer(search).await
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, ApiError> {
        self.galoy().list_offers(search).await
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, ApiError> {
        self.galoy()
            .pay_offer(offer, amount_msats, payer_note)
            .await
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<Transaction, ApiError> {
        self.galoy().lookup_invoice(params).await
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, ApiError> {
        self.galoy().list_transactions(params).await
    }

    pub async fn decode(&self, value: String) -> Result<String, ApiError> {
        self.galoy().decode(value).await
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, ApiError> {
        self.galoy().decode_offer(offer).await
    }

    pub async fn on_invoice_events(
        &self,
        params: crate::types::OnInvoiceEventParams,
        callback: std::sync::Arc<dyn crate::types::OnInvoiceEventCallback>,
    ) {
        self.galoy().on_invoice_events(params, callback).await
    }
}

crate::impl_lightning_node!(FlashNode);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galoy::{
        GaloyInvoiceOperation, GaloyPaymentResponse, GaloyPermissionsMode, GaloyWalletConfig,
    };

    fn config() -> FlashConfig {
        FlashConfig {
            api_key: "top-secret-api-key".to_string(),
            base_url: None,
            wallet_id: "wallet-usd".to_string(),
            wallet_currency: "USD".to_string(),
            additional_headers: None,
            accepted_statuses: None,
            http_timeout: Some(60),
            socks5_proxy: None,
            accept_invalid_certs: Some(false),
        }
    }

    #[test]
    fn supplies_flash_galoy_defaults() {
        let galoy = crate::galoy::GaloyConfig::from(&config());
        assert_eq!(galoy.provider.id, "flash");
        assert_eq!(galoy.provider.name, "Flash");
        assert_eq!(galoy.base_url, DEFAULT_FLASH_GRAPHQL_URL);
        assert_eq!(
            galoy.wallet,
            GaloyWalletConfig::Explicit {
                id: "wallet-usd".to_string(),
                currency: "USD".to_string(),
            }
        );
        assert_eq!(
            galoy.invoice_operations.create,
            GaloyInvoiceOperation::Unsupported
        );
        assert_eq!(
            galoy.invoice_operations.fee_probe,
            GaloyInvoiceOperation::UsdCents
        );
        assert_eq!(galoy.payment.response, GaloyPaymentResponse::StatusOnly);
        assert_eq!(
            galoy.payment.accepted_statuses,
            ["SUCCESS", "PENDING", "ALREADY_PAID"]
        );
        assert_eq!(galoy.permissions, GaloyPermissionsMode::Configured);
        assert!(!galoy.capabilities.transaction_lookup);
        assert!(!galoy.capabilities.transaction_history);
        assert!(!galoy.capabilities.invoice_events);
        assert!(!galoy.capabilities.onchain);
    }

    #[tokio::test]
    async fn reports_invoice_creation_as_unsupported() {
        let node = FlashNode::new(config());
        let permissions = node
            .get_permissions()
            .await
            .expect("configured permissions should resolve");
        assert!(!permissions.create_invoice);

        let error = node
            .create_invoice(CreateInvoiceParams {
                amount_msats: Some(123_000),
                ..Default::default()
            })
            .await
            .expect_err("Flash invoice creation must be disabled");
        assert!(matches!(
            error,
            ApiError::Nwc { ref code, ref message }
                if code == "NOT_IMPLEMENTED" && message.contains("[flash]")
        ));
    }

    #[test]
    fn arbitrary_non_btc_wallet_does_not_select_usd_operations() {
        let mut config = config();
        config.wallet_currency = "JMD".to_string();
        let galoy = crate::galoy::GaloyConfig::from(&config);
        assert_eq!(
            galoy.invoice_operations.fee_probe,
            GaloyInvoiceOperation::Unsupported
        );
    }

    #[test]
    fn allows_the_default_endpoint_to_be_overridden() {
        let mut config = config();
        config.base_url = Some("https://flash.test/custom-graphql".to_string());
        let galoy = crate::galoy::GaloyConfig::from(&config);
        assert_eq!(galoy.base_url, "https://flash.test/custom-graphql");
    }

    #[test]
    fn debug_redacts_credentials_and_header_values() {
        let mut config = config();
        config.additional_headers = Some(HashMap::from([(
            "x-flash-secret".to_string(),
            "header-secret".to_string(),
        )]));
        let debug = format!("{config:?}");
        assert!(!debug.contains("top-secret-api-key"));
        assert!(!debug.contains("header-secret"));
        assert!(debug.contains("x-flash-secret"));
    }
}
