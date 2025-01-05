pub trait ILightningNode {
    fn get_wallet_transactions(&self, wallet_id: &str) -> Result<Vec<Transaction>, String>;
    fn pay_invoice(&self, invoice: &str) -> Result<String, String>;
    fn get_bolt12_offer(&self) -> Result<String, String>;
    fn fetch_wallet_balance(&self) -> Result<FetchWalletBalanceResponseType, String>;
    fn decode_invoice(&self, invoice: &str) -> Result<Invoice, String>;
    fn check_payment_status(&self, payment_id: &str) -> Result<PaymentStatus, String>;
    fn fetch_channel_info(&self, channel_id: &str) -> Result<FetchChannelInfoResponseType, String>;
    fn on_payment_received(&self, event: &str);
}

pub struct FetchWalletBalanceResponseType {
    pub balance: u64,
}

pub struct FetchChannelInfoResponseType {
    pub send: u64,
    pub receive: u64,
}

pub struct Transaction {
    pub amount: i64,
    pub date: String,
    pub memo: String,
}

pub struct Invoice {
    pub amount: u64,
    pub memo: String,
}

pub struct PaymentStatus {
    pub status: String,
}

pub enum WalletProviderType {
    Phoenixd,
    Lndk,
    CoreLightning,
    Strike,
    None,
}