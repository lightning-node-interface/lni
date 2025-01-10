use wasm_bindgen::prelude::*;
use crate::interface::{
    WalletInterface, Transaction, FetchWalletBalanceResponseType,
    FetchChannelInfoResponseType, PaymentStatus,
};

#[wasm_bindgen]
pub struct ClnNode {
    rune: String,
    url: String,
    wallet_interface: WalletInterface,
}

#[wasm_bindgen]
impl ClnNode {
    #[wasm_bindgen(constructor)]
    pub fn new(rune: String, url: String) -> ClnNode {
        ClnNode {
            rune,
            url,
            wallet_interface: WalletInterface::CLN_REST,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn key(&self) -> String {
        self.rune.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn url(&self) -> String {
        self.url.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn wallet_interface(&self) -> WalletInterface {
        self.wallet_interface.clone()
    }

    pub fn fetch_wallet_balance(&self) -> FetchWalletBalanceResponseType {
        FetchWalletBalanceResponseType::new(1000)
    }

    pub fn fetch_channel_info(&self, channel_id: String) -> FetchChannelInfoResponseType {
        FetchChannelInfoResponseType::new(100, 50)
    }

    pub fn check_payment_status(&self, payment_id: String) -> PaymentStatus {
        PaymentStatus::new("PAID".to_string())
    }

    pub fn get_wallet_transactions(&self, wallet_id: String) -> Vec<Transaction> {
        vec![
            Transaction::new(100, "2023-01-01".to_string(), "Payment from Bob".to_string()),
            Transaction::new(-50, "2023-01-02".to_string(), "Payment to Alice".to_string()),
        ]
    }

    pub fn pay_invoice(&self, invoice: String) -> String {
        format!("Paid invoice: {}", invoice)
    }

    pub fn get_bolt12_offer(&self) -> String {
        "lno".to_string()
    }

    pub fn on_payment_received(&self, event: String) {
        log::info!("Payment received: {}", event);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cln_payment() {
        let cln_node = ClnNode::new("test_rune".to_string(), "https://127.0.0.1:8081".to_string());
        let result =  cln_node.pay_invoice("invoice".to_string());
        assert!(!result.is_empty());
    }
}