use crate::{interface::{
    FetchChannelInfoResponseType, FetchWalletBalanceResponseType, ILightningNode, Invoice,
    PaymentStatus, Transaction,
}, LightningConfig, WalletInterface};

pub struct LndConfig {
    pub macaroon: String,
    pub url: String,
    pub wallet_interface: WalletInterface,
}
pub struct LndNode {
    config: LndConfig,
}

impl LightningConfig for LndConfig {
    fn get_url(&self) -> &str {
        &self.url
    }
    fn get_key(&self) -> &str {
        &self.macaroon
    }
    fn get_type(&self) -> WalletInterface {
        WalletInterface::LND_REST
    }
}

impl ILightningNode for LndNode {
    // config and constructor
    type Config = LndConfig;
    fn new(config: Self::Config) -> Self {
        Self { config }
    }

    // methods
    fn get_wallet_transactions(&self, wallet_id: &str) -> Result<Vec<Transaction>, String> {
        Ok(vec![
            Transaction {
                amount: 100,
                date: "2023-01-01".to_string(),
                memo: "Payment from Bob".to_string(),
            },
            Transaction {
                amount: -50,
                date: "2023-01-02".to_string(),
                memo: "Payment to Alice".to_string(),
            },
        ])
    }

    fn pay_invoice(&self, invoice: &str) -> Result<String, String> {
        Ok("Payment successful".to_string())
    }

    fn get_bolt12_offer(&self) -> Result<String, String> {
        Ok("lno".to_string())
    }

    fn fetch_wallet_balance(&self) -> Result<FetchWalletBalanceResponseType, String> {
        Ok(FetchWalletBalanceResponseType { balance: 1000 })
    }

    fn decode_invoice(&self, invoice: &str) -> Result<Invoice, String> {
        Ok(Invoice {
            amount: 100,
            memo: "Payment from Bob".to_string(),
        })
    }

    fn check_payment_status(&self, payment_id: &str) -> Result<PaymentStatus, String> {
        Ok(PaymentStatus {
            status: "PAID".to_string(),
        })
    }

    fn fetch_channel_info(&self, channel_id: &str) -> Result<FetchChannelInfoResponseType, String> {
        Ok(FetchChannelInfoResponseType {
            send: 100,
            receive: 50,
        })
    }

    fn on_payment_received(&self, event: &str) {
        // 1. verify payment
        // 2. write to payment-received file in tor data directory
        //    paymentHash | expires(null) | amount
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lnd_payment() {
        let config = LndConfig {
            macaroon: "test_macaroon".to_string(),
            url: "https://127.0.0.1:8080".to_string(),
            wallet_interface: WalletInterface::LND_REST,
        };
        let node = LndNode::new(config);
        let result = node.pay_invoice("test_invoice");
        assert!(result.is_ok());
    }
}