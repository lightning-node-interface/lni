use wasm_bindgen::prelude::*;

// Enum representing different wallet interfaces
#[wasm_bindgen]
#[derive(Clone, Copy)]

pub enum WalletInterface {
    LND_REST,
    CLN_REST,
    PHOENIXD_REST,
}

// Struct representing a transaction
#[wasm_bindgen]
pub struct Transaction {
    amount: i64,
    date: String,
    memo: String,
}

#[wasm_bindgen]
impl Transaction {
    #[wasm_bindgen(constructor)]
    pub fn new(amount: i64, date: String, memo: String) -> Transaction {
        Transaction { amount, date, memo }
    }

    #[wasm_bindgen(getter)]
    pub fn amount(&self) -> i64 {
        self.amount
    }

    #[wasm_bindgen(getter)]
    pub fn date(&self) -> String {
        self.date.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn memo(&self) -> String {
        self.memo.clone()
    }
}

// Struct for fetching wallet balance response
#[wasm_bindgen]
pub struct FetchWalletBalanceResponseType {
    balance: u64,
}

#[wasm_bindgen]
impl FetchWalletBalanceResponseType {
    #[wasm_bindgen(constructor)]
    pub fn new(balance: u64) -> FetchWalletBalanceResponseType {
        FetchWalletBalanceResponseType { balance }
    }

    #[wasm_bindgen(getter)]
    pub fn balance(&self) -> u64 {
        self.balance
    }
}

// Struct for fetching channel info response
#[wasm_bindgen]
pub struct FetchChannelInfoResponseType {
    send: u64,
    receive: u64,
}

#[wasm_bindgen]
impl FetchChannelInfoResponseType {
    #[wasm_bindgen(constructor)]
    pub fn new(send: u64, receive: u64) -> FetchChannelInfoResponseType {
        FetchChannelInfoResponseType { send, receive }
    }

    #[wasm_bindgen(getter)]
    pub fn send(&self) -> u64 {
        self.send
    }

    #[wasm_bindgen(getter)]
    pub fn receive(&self) -> u64 {
        self.receive
    }
}

// Struct for payment status
#[wasm_bindgen]
pub struct PaymentStatus {
    status: String,
}

#[wasm_bindgen]
impl PaymentStatus {
    #[wasm_bindgen(constructor)]
    pub fn new(status: String) -> PaymentStatus {
        PaymentStatus { status }
    }

    #[wasm_bindgen(getter)]
    pub fn status(&self) -> String {
        self.status.clone()
    }
}
