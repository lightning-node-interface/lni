#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    amount: i64,
    date: String,
    memo: String,
}

impl Transaction {
    pub fn new(amount: i64, date: String, memo: String) -> Transaction {
        Transaction { amount, date, memo }
    }

    // Getters for each field
    pub fn amount(&self) -> i64 {
        self.amount
    }

    pub fn date(&self) -> String {
        self.date.clone()
    }

    pub fn memo(&self) -> String {
        self.memo.clone()
    }
}
