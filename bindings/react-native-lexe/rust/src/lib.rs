use std::sync::Arc;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LexeError {
    #[error("{message}")]
    Lni { message: String },
}

impl From<lni::ApiError> for LexeError {
    fn from(error: lni::ApiError) -> Self {
        Self::Lni {
            message: error.to_string(),
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct LexeConfig {
    pub client_credentials: String,
    pub data_dir: Option<String>,
    pub network: Option<String>,
}

impl From<LexeConfig> for lni::lexe::LexeConfig {
    fn from(config: LexeConfig) -> Self {
        Self {
            client_credentials: config.client_credentials,
            data_dir: config.data_dir,
            network: config.network,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Permissions {
    pub get_info: bool,
    pub create_invoice: bool,
    pub pay_invoice: bool,
    pub create_offer: bool,
    pub get_offer: bool,
    pub list_offers: bool,
    pub pay_offer: bool,
    pub lookup_invoice: bool,
    pub list_transactions: bool,
    pub decode: bool,
    pub on_invoice_events: bool,
}

impl From<lni::Permissions> for Permissions {
    fn from(value: lni::Permissions) -> Self {
        Self {
            get_info: value.get_info,
            create_invoice: value.create_invoice,
            pay_invoice: value.pay_invoice,
            create_offer: value.create_offer,
            get_offer: value.get_offer,
            list_offers: value.list_offers,
            pay_offer: value.pay_offer,
            lookup_invoice: value.lookup_invoice,
            list_transactions: value.list_transactions,
            decode: value.decode,
            on_invoice_events: value.on_invoice_events,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct NodeInfo {
    pub alias: String,
    pub color: String,
    pub pubkey: String,
    pub network: String,
    pub block_height: i64,
    pub block_hash: String,
    pub send_balance_msat: i64,
    pub receive_balance_msat: i64,
    pub fee_credit_balance_msat: i64,
    pub unsettled_send_balance_msat: i64,
    pub unsettled_receive_balance_msat: i64,
    pub pending_open_send_balance: i64,
    pub pending_open_receive_balance: i64,
}

impl From<lni::NodeInfo> for NodeInfo {
    fn from(value: lni::NodeInfo) -> Self {
        Self {
            alias: value.alias,
            color: value.color,
            pubkey: value.pubkey,
            network: value.network,
            block_height: value.block_height,
            block_hash: value.block_hash,
            send_balance_msat: value.send_balance_msat,
            receive_balance_msat: value.receive_balance_msat,
            fee_credit_balance_msat: value.fee_credit_balance_msat,
            unsettled_send_balance_msat: value.unsettled_send_balance_msat,
            unsettled_receive_balance_msat: value.unsettled_receive_balance_msat,
            pending_open_send_balance: value.pending_open_send_balance,
            pending_open_receive_balance: value.pending_open_receive_balance,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct ClientInfo {
    pub kind: String,
    pub client_pubkey: Option<String>,
    pub label: Option<String>,
    pub created_at_ms: Option<i64>,
    pub expires_at_ms: Option<i64>,
    pub scopes: Vec<String>,
    pub permissions: Vec<String>,
    pub effective_permissions: Vec<String>,
}

impl From<lni::lexe::LexeClientInfo> for ClientInfo {
    fn from(value: lni::lexe::LexeClientInfo) -> Self {
        Self {
            kind: value.kind,
            client_pubkey: value.client_pubkey,
            label: value.label,
            created_at_ms: value.created_at_ms,
            expires_at_ms: value.expires_at_ms,
            scopes: value.scopes,
            permissions: value.permissions,
            effective_permissions: value.effective_permissions,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct HumanBitcoinAddress {
    pub human_bitcoin_address: String,
    pub lightning_address: String,
    pub offer: String,
    pub updatable: bool,
}

impl From<lni::lexe::LexeHumanBitcoinAddress> for HumanBitcoinAddress {
    fn from(value: lni::lexe::LexeHumanBitcoinAddress) -> Self {
        Self {
            human_bitcoin_address: value.human_bitcoin_address,
            lightning_address: value.lightning_address,
            offer: value.offer,
            updatable: value.updatable,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Transaction {
    pub type_: String,
    pub invoice: String,
    pub description: String,
    pub description_hash: String,
    pub preimage: String,
    pub payment_hash: String,
    pub amount_msats: i64,
    pub fees_paid: i64,
    pub created_at: i64,
    pub expires_at: i64,
    pub settled_at: i64,
    pub payer_note: Option<String>,
    pub external_id: Option<String>,
}

impl From<lni::Transaction> for Transaction {
    fn from(value: lni::Transaction) -> Self {
        Self {
            type_: value.type_,
            invoice: value.invoice,
            description: value.description,
            description_hash: value.description_hash,
            preimage: value.preimage,
            payment_hash: value.payment_hash,
            amount_msats: value.amount_msats,
            fees_paid: value.fees_paid,
            created_at: value.created_at,
            expires_at: value.expires_at,
            settled_at: value.settled_at,
            payer_note: value.payer_note,
            external_id: value.external_id,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct PayInvoiceResponse {
    pub payment_hash: String,
    pub preimage: String,
    pub fee_msats: i64,
}

impl From<lni::PayInvoiceResponse> for PayInvoiceResponse {
    fn from(value: lni::PayInvoiceResponse) -> Self {
        Self {
            payment_hash: value.payment_hash,
            preimage: value.preimage,
            fee_msats: value.fee_msats,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct Offer {
    pub offer_id: String,
    pub bolt12: String,
    pub label: Option<String>,
    pub active: Option<bool>,
    pub single_use: Option<bool>,
    pub used: Option<bool>,
    pub amount_msats: Option<i64>,
}

impl From<lni::Offer> for Offer {
    fn from(value: lni::Offer) -> Self {
        Self {
            offer_id: value.offer_id,
            bolt12: value.bolt12,
            label: value.label,
            active: value.active,
            single_use: value.single_use,
            used: value.used,
            amount_msats: value.amount_msats,
        }
    }
}

#[derive(Clone, uniffi::Enum)]
pub enum InvoiceType {
    Bolt11,
    Bolt12,
}

impl From<InvoiceType> for lni::InvoiceType {
    fn from(value: InvoiceType) -> Self {
        match value {
            InvoiceType::Bolt11 => Self::Bolt11,
            InvoiceType::Bolt12 => Self::Bolt12,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct CreateInvoiceParams {
    pub invoice_type: Option<InvoiceType>,
    pub amount_msats: Option<i64>,
    pub offer: Option<String>,
    pub description: Option<String>,
    pub description_hash: Option<String>,
    pub expiry: Option<i64>,
    pub r_preimage: Option<String>,
    pub is_blinded: Option<bool>,
    pub is_keysend: Option<bool>,
    pub is_amp: Option<bool>,
    pub is_private: Option<bool>,
}

impl From<CreateInvoiceParams> for lni::CreateInvoiceParams {
    fn from(value: CreateInvoiceParams) -> Self {
        Self {
            invoice_type: value.invoice_type.map(Into::into),
            amount_msats: value.amount_msats,
            offer: value.offer,
            description: value.description,
            description_hash: value.description_hash,
            expiry: value.expiry,
            r_preimage: value.r_preimage,
            is_blinded: value.is_blinded,
            is_keysend: value.is_keysend,
            is_amp: value.is_amp,
            is_private: value.is_private,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct PayInvoiceParams {
    pub invoice: String,
    pub fee_limit_msat: Option<i64>,
    pub fee_limit_percentage: Option<f64>,
    pub timeout_seconds: Option<i64>,
    pub amount_msats: Option<i64>,
    pub max_parts: Option<i64>,
    pub first_hop_pubkey: Option<String>,
    pub last_hop_pubkey: Option<String>,
    pub allow_self_payment: Option<bool>,
    pub is_amp: Option<bool>,
}

impl From<PayInvoiceParams> for lni::PayInvoiceParams {
    fn from(value: PayInvoiceParams) -> Self {
        Self {
            invoice: value.invoice,
            fee_limit_msat: value.fee_limit_msat,
            fee_limit_percentage: value.fee_limit_percentage,
            timeout_seconds: value.timeout_seconds,
            amount_msats: value.amount_msats,
            max_parts: value.max_parts,
            first_hop_pubkey: value.first_hop_pubkey,
            last_hop_pubkey: value.last_hop_pubkey,
            allow_self_payment: value.allow_self_payment,
            is_amp: value.is_amp,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct CreateOfferParams {
    pub description: Option<String>,
    pub amount_msats: Option<i64>,
}

impl From<CreateOfferParams> for lni::CreateOfferParams {
    fn from(value: CreateOfferParams) -> Self {
        Self {
            description: value.description,
            amount_msats: value.amount_msats,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct LookupInvoiceParams {
    pub payment_hash: Option<String>,
    pub search: Option<String>,
}

impl From<LookupInvoiceParams> for lni::LookupInvoiceParams {
    fn from(value: LookupInvoiceParams) -> Self {
        Self {
            payment_hash: value.payment_hash,
            search: value.search,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct ListTransactionsParams {
    pub from: i64,
    pub limit: i64,
    pub payment_hash: Option<String>,
    pub search: Option<String>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
}

impl From<ListTransactionsParams> for lni::ListTransactionsParams {
    fn from(value: ListTransactionsParams) -> Self {
        Self {
            from: value.from,
            limit: value.limit,
            payment_hash: value.payment_hash,
            search: value.search,
            created_after: value.created_after,
            created_before: value.created_before,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct OnInvoiceEventParams {
    pub payment_hash: Option<String>,
    pub search: Option<String>,
    pub polling_delay_sec: i64,
    pub max_polling_sec: i64,
}

impl From<OnInvoiceEventParams> for lni::OnInvoiceEventParams {
    fn from(value: OnInvoiceEventParams) -> Self {
        Self {
            payment_hash: value.payment_hash,
            search: value.search,
            polling_delay_sec: value.polling_delay_sec,
            max_polling_sec: value.max_polling_sec,
        }
    }
}

#[uniffi::export(with_foreign)]
pub trait OnInvoiceEventCallback: Send + Sync {
    fn success(&self, transaction: Option<Transaction>);
    fn pending(&self, transaction: Option<Transaction>);
    fn failure(&self, transaction: Option<Transaction>);
}

struct CallbackAdapter {
    callback: Arc<dyn OnInvoiceEventCallback>,
}

impl lni::OnInvoiceEventCallback for CallbackAdapter {
    fn success(&self, transaction: Option<lni::Transaction>) {
        self.callback.success(transaction.map(Into::into));
    }

    fn pending(&self, transaction: Option<lni::Transaction>) {
        self.callback.pending(transaction.map(Into::into));
    }

    fn failure(&self, transaction: Option<lni::Transaction>) {
        self.callback.failure(transaction.map(Into::into));
    }
}

#[derive(uniffi::Object)]
pub struct LexeNode {
    inner: lni::lexe::LexeNode,
}

#[uniffi::export]
impl LexeNode {
    #[uniffi::constructor]
    pub fn new(config: LexeConfig) -> Result<Self, LexeError> {
        Ok(Self {
            inner: lni::lexe::LexeNode::new(config.into())?,
        })
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl LexeNode {
    pub async fn get_permissions(&self) -> Result<Permissions, LexeError> {
        Ok(self.inner.get_permissions().await?.into())
    }

    pub async fn get_client_info(&self) -> Result<ClientInfo, LexeError> {
        Ok(self.inner.get_client_info().await?.into())
    }

    pub async fn get_info(&self) -> Result<NodeInfo, LexeError> {
        Ok(self.inner.get_info().await?.into())
    }

    pub async fn get_human_bitcoin_address(&self) -> Result<HumanBitcoinAddress, LexeError> {
        Ok(self.inner.get_human_bitcoin_address().await?.into())
    }

    pub async fn create_invoice(
        &self,
        params: CreateInvoiceParams,
    ) -> Result<Transaction, LexeError> {
        Ok(self.inner.create_invoice(params.into()).await?.into())
    }

    pub async fn pay_invoice(
        &self,
        params: PayInvoiceParams,
    ) -> Result<PayInvoiceResponse, LexeError> {
        Ok(self.inner.pay_invoice(params.into()).await?.into())
    }

    pub async fn prepare_onchain_transaction(
        &self,
        params: PrepareOnchainTransactionParams,
    ) -> Result<OnchainTransaction, LexeError> {
        Ok(self
            .inner
            .prepare_onchain_transaction(params.into())
            .await?
            .into())
    }

    pub async fn pay_onchain(
        &self,
        transaction: OnchainTransaction,
        options: PayOnchainOptions,
    ) -> Result<PayOnchainResponse, LexeError> {
        Ok(self
            .inner
            .pay_onchain_with_options(transaction.into(), options.into())
            .await?
            .into())
    }

    pub async fn create_offer(&self, params: CreateOfferParams) -> Result<Offer, LexeError> {
        Ok(self.inner.create_offer(params.into()).await?.into())
    }

    pub async fn get_offer(&self, search: Option<String>) -> Result<Offer, LexeError> {
        Ok(self.inner.get_offer(search).await?.into())
    }

    pub async fn list_offers(&self, search: Option<String>) -> Result<Vec<Offer>, LexeError> {
        Ok(self
            .inner
            .list_offers(search)
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn pay_offer(
        &self,
        offer: String,
        amount_msats: i64,
        payer_note: Option<String>,
    ) -> Result<PayInvoiceResponse, LexeError> {
        Ok(self
            .inner
            .pay_offer(offer, amount_msats, payer_note)
            .await?
            .into())
    }

    pub async fn lookup_invoice(
        &self,
        params: LookupInvoiceParams,
    ) -> Result<Transaction, LexeError> {
        Ok(self.inner.lookup_invoice(params.into()).await?.into())
    }

    pub async fn list_transactions(
        &self,
        params: ListTransactionsParams,
    ) -> Result<Vec<Transaction>, LexeError> {
        Ok(self
            .inner
            .list_transactions(params.into())
            .await?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub async fn decode(&self, value: String) -> Result<String, LexeError> {
        Ok(self.inner.decode(value).await?)
    }

    pub async fn decode_offer(&self, offer: String) -> Result<String, LexeError> {
        Ok(self.inner.decode_offer(offer).await?)
    }

    pub async fn on_invoice_events(
        &self,
        params: OnInvoiceEventParams,
        callback: Arc<dyn OnInvoiceEventCallback>,
    ) {
        self.inner
            .on_invoice_events(params.into(), Arc::new(CallbackAdapter { callback }))
            .await;
    }
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_credentials() {
        let result = LexeNode::new(LexeConfig {
            client_credentials: String::new(),
            data_dir: None,
            network: Some("mainnet".to_owned()),
        });

        assert!(result.is_err());
    }
}

#[derive(Clone, uniffi::Enum)]
pub enum OnchainFeeSpeed {
    Fast,
    Normal,
    Slow,
    Free,
}

impl From<OnchainFeeSpeed> for lni::OnchainFeeSpeed {
    fn from(value: OnchainFeeSpeed) -> Self {
        match value {
            OnchainFeeSpeed::Fast => Self::Fast,
            OnchainFeeSpeed::Normal => Self::Normal,
            OnchainFeeSpeed::Slow => Self::Slow,
            OnchainFeeSpeed::Free => Self::Free,
        }
    }
}

impl From<lni::OnchainFeeSpeed> for OnchainFeeSpeed {
    fn from(value: lni::OnchainFeeSpeed) -> Self {
        match value {
            lni::OnchainFeeSpeed::Fast => Self::Fast,
            lni::OnchainFeeSpeed::Normal => Self::Normal,
            lni::OnchainFeeSpeed::Slow => Self::Slow,
            lni::OnchainFeeSpeed::Free => Self::Free,
        }
    }
}

#[derive(Clone, uniffi::Enum)]
pub enum OnchainFeePayer {
    Sender,
    Recipient,
}

impl From<OnchainFeePayer> for lni::OnchainFeePayer {
    fn from(value: OnchainFeePayer) -> Self {
        match value {
            OnchainFeePayer::Sender => Self::Sender,
            OnchainFeePayer::Recipient => Self::Recipient,
        }
    }
}

impl From<lni::OnchainFeePayer> for OnchainFeePayer {
    fn from(value: lni::OnchainFeePayer) -> Self {
        match value {
            lni::OnchainFeePayer::Sender => Self::Sender,
            lni::OnchainFeePayer::Recipient => Self::Recipient,
        }
    }
}

#[derive(Clone, uniffi::Enum)]
pub enum OnchainFeePreferenceType {
    Default,
    Speed,
    TargetConf,
    SatsPerVbyte,
    Backend,
}

impl From<OnchainFeePreferenceType> for lni::OnchainFeePreferenceType {
    fn from(value: OnchainFeePreferenceType) -> Self {
        match value {
            OnchainFeePreferenceType::Default => Self::Default,
            OnchainFeePreferenceType::Speed => Self::Speed,
            OnchainFeePreferenceType::TargetConf => Self::TargetConf,
            OnchainFeePreferenceType::SatsPerVbyte => Self::SatsPerVbyte,
            OnchainFeePreferenceType::Backend => Self::Backend,
        }
    }
}

impl From<lni::OnchainFeePreferenceType> for OnchainFeePreferenceType {
    fn from(value: lni::OnchainFeePreferenceType) -> Self {
        match value {
            lni::OnchainFeePreferenceType::Default => Self::Default,
            lni::OnchainFeePreferenceType::Speed => Self::Speed,
            lni::OnchainFeePreferenceType::TargetConf => Self::TargetConf,
            lni::OnchainFeePreferenceType::SatsPerVbyte => Self::SatsPerVbyte,
            lni::OnchainFeePreferenceType::Backend => Self::Backend,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct OnchainFeePreference {
    pub preference_type: OnchainFeePreferenceType,
    pub speed: Option<OnchainFeeSpeed>,
    pub target_conf: Option<i64>,
    pub sats_per_vbyte: Option<f64>,
    pub backend: Option<String>,
}

impl From<OnchainFeePreference> for lni::OnchainFeePreference {
    fn from(value: OnchainFeePreference) -> Self {
        Self {
            preference_type: value.preference_type.into(),
            speed: value.speed.map(Into::into),
            target_conf: value.target_conf,
            sats_per_vbyte: value.sats_per_vbyte,
            backend: value.backend,
        }
    }
}

impl From<lni::OnchainFeePreference> for OnchainFeePreference {
    fn from(value: lni::OnchainFeePreference) -> Self {
        Self {
            preference_type: value.preference_type.into(),
            speed: value.speed.map(Into::into),
            target_conf: value.target_conf,
            sats_per_vbyte: value.sats_per_vbyte,
            backend: value.backend,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct PrepareOnchainTransactionParams {
    pub address: String,
    pub amount_sats: i64,
    pub fee: Option<OnchainFeePreference>,
    pub fee_payer: Option<OnchainFeePayer>,
    pub description: Option<String>,
    pub idempotency_key: Option<String>,
}

impl From<PrepareOnchainTransactionParams> for lni::PrepareOnchainTransactionParams {
    fn from(value: PrepareOnchainTransactionParams) -> Self {
        Self {
            address: value.address,
            amount_sats: value.amount_sats,
            fee: value.fee.map(Into::into),
            fee_payer: value.fee_payer.map(Into::into),
            description: value.description,
            idempotency_key: value.idempotency_key,
        }
    }
}

impl From<lni::PrepareOnchainTransactionParams> for PrepareOnchainTransactionParams {
    fn from(value: lni::PrepareOnchainTransactionParams) -> Self {
        Self {
            address: value.address,
            amount_sats: value.amount_sats,
            fee: value.fee.map(Into::into),
            fee_payer: value.fee_payer.map(Into::into),
            description: value.description,
            idempotency_key: value.idempotency_key,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct OnchainTransaction {
    pub id: Option<String>,
    pub fee_limit_supported: Option<bool>,
    pub address: String,
    pub amount_sats: i64,
    pub fee_sats: Option<i64>,
    pub total_amount_sats: Option<i64>,
    pub recipient_amount_sats: Option<i64>,
    pub fee_payer: OnchainFeePayer,
    pub fee: OnchainFeePreference,
    pub expires_at: Option<i64>,
    pub estimated_delivery_seconds: Option<i64>,
    pub raw: Option<String>,
}

impl From<OnchainTransaction> for lni::OnchainTransaction {
    fn from(value: OnchainTransaction) -> Self {
        Self {
            id: value.id,
            fee_limit_supported: value.fee_limit_supported,
            address: value.address,
            amount_sats: value.amount_sats,
            fee_sats: value.fee_sats,
            total_amount_sats: value.total_amount_sats,
            recipient_amount_sats: value.recipient_amount_sats,
            fee_payer: value.fee_payer.into(),
            fee: value.fee.into(),
            expires_at: value.expires_at,
            estimated_delivery_seconds: value.estimated_delivery_seconds,
            raw: value.raw,
        }
    }
}

impl From<lni::OnchainTransaction> for OnchainTransaction {
    fn from(value: lni::OnchainTransaction) -> Self {
        Self {
            id: value.id,
            fee_limit_supported: value.fee_limit_supported,
            address: value.address,
            amount_sats: value.amount_sats,
            fee_sats: value.fee_sats,
            total_amount_sats: value.total_amount_sats,
            recipient_amount_sats: value.recipient_amount_sats,
            fee_payer: value.fee_payer.into(),
            fee: value.fee.into(),
            expires_at: value.expires_at,
            estimated_delivery_seconds: value.estimated_delivery_seconds,
            raw: value.raw,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct OnchainFeeGuardrail {
    pub max_fee_sats: Option<i64>,
    pub max_fee_percent: Option<f64>,
}

impl From<OnchainFeeGuardrail> for lni::OnchainFeeGuardrail {
    fn from(value: OnchainFeeGuardrail) -> Self {
        Self {
            max_fee_sats: value.max_fee_sats,
            max_fee_percent: value.max_fee_percent,
        }
    }
}

impl From<lni::OnchainFeeGuardrail> for OnchainFeeGuardrail {
    fn from(value: lni::OnchainFeeGuardrail) -> Self {
        Self {
            max_fee_sats: value.max_fee_sats,
            max_fee_percent: value.max_fee_percent,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct PayOnchainOptions {
    pub fee_guardrail: Option<OnchainFeeGuardrail>,
    pub dangerously_disable_fee_guardrail: bool,
}

impl From<PayOnchainOptions> for lni::PayOnchainOptions {
    fn from(value: PayOnchainOptions) -> Self {
        Self {
            fee_guardrail: value.fee_guardrail.map(Into::into),
            dangerously_disable_fee_guardrail: value.dangerously_disable_fee_guardrail,
        }
    }
}

impl From<lni::PayOnchainOptions> for PayOnchainOptions {
    fn from(value: lni::PayOnchainOptions) -> Self {
        Self {
            fee_guardrail: value.fee_guardrail.map(Into::into),
            dangerously_disable_fee_guardrail: value.dangerously_disable_fee_guardrail,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct PayOnchainResponse {
    pub payment_id: Option<String>,
    pub txid: Option<String>,
    pub state: String,
    pub address: String,
    pub amount_sats: i64,
    pub fee_sats: Option<i64>,
    pub total_amount_sats: Option<i64>,
    pub recipient_amount_sats: Option<i64>,
    pub created_at: Option<i64>,
    pub raw: Option<String>,
}

impl From<PayOnchainResponse> for lni::PayOnchainResponse {
    fn from(value: PayOnchainResponse) -> Self {
        Self {
            payment_id: value.payment_id,
            txid: value.txid,
            state: value.state,
            address: value.address,
            amount_sats: value.amount_sats,
            fee_sats: value.fee_sats,
            total_amount_sats: value.total_amount_sats,
            recipient_amount_sats: value.recipient_amount_sats,
            created_at: value.created_at,
            raw: value.raw,
        }
    }
}

impl From<lni::PayOnchainResponse> for PayOnchainResponse {
    fn from(value: lni::PayOnchainResponse) -> Self {
        Self {
            payment_id: value.payment_id,
            txid: value.txid,
            state: value.state,
            address: value.address,
            amount_sats: value.amount_sats,
            fee_sats: value.fee_sats,
            total_amount_sats: value.total_amount_sats,
            recipient_amount_sats: value.recipient_amount_sats,
            created_at: value.created_at,
            raw: value.raw,
        }
    }
}
