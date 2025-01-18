use crate::Transaction;
use tokio::time::{sleep, Duration};
use rand::{distributions::Alphanumeric, Rng};


pub async fn get_wallet_transactions() -> Vec<Transaction> {
    sleep(Duration::from_secs(1)).await;
    vec![
        Transaction::new(100, "2023-01-01".into(), "Payment from Bob".into()),
        Transaction::new(-50, "2023-01-02".into(), "Payment to Alice".into()),
    ]
}

pub async fn create_invoice(amount: u64, memo: String) -> String {
    sleep(Duration::from_secs(1)).await;
    let random_string: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    format!("lnp{}{}", amount.to_string(), random_string)
}


pub async fn get_invoice(payment_id: String) -> String {
    sleep(Duration::from_secs(1)).await;
    let random_string: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    format!("lnp{}", random_string)
}

