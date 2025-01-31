use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("IoError: {reason}")]
    IoErr { reason: String },
    #[error("SerializationError: {reason}")]
    SerializationErr { reason: String },
    #[error("DeserializationError: {reason}")]
    DeserializationErr { reason: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Payment {
    pub payment_id: String,
    pub circ_id: String,
    pub round: i64,
    pub relay_fingerprint: String,
    pub updated_at: i64,
    pub amount_msat: i64,
}

#[derive(Debug, Deserialize)]
pub struct Db {
    path: String,
    #[serde(skip)]
    data: Arc<Mutex<Vec<Payment>>>,
}

impl Db {
    pub fn new(path: String) -> Result<Self, DbError> {
        let data = if let Ok(mut file) = File::open(&path) {
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .map_err(|e| DbError::IoErr {
                    reason: e.to_string(),
                })?;
            serde_json::from_str(&contents).map_err(|e| DbError::DeserializationErr {
                reason: e.to_string(),
            })?
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            data: Arc::new(Mutex::new(data)),
        })
    }

    pub fn save(&self) -> Result<(), DbError> {
        let data = self.data.lock().unwrap();
        let json = serde_json::to_string_pretty(&*data).map_err(|e| DbError::SerializationErr {
            reason: e.to_string(),
        })?;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|e| DbError::IoErr {
                reason: e.to_string(),
            })?;
        file.write_all(json.as_bytes())
            .map_err(|e| DbError::IoErr {
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub fn write_payment(&self, payment: Payment) -> Result<(), DbError> {
        let mut data = self.data.lock().unwrap();
        data.push(payment);
        drop(data); // Explicitly drop the lock before saving
        self.save()
    }
 
    pub fn lookup_payment(&self, payment_id: String) -> Result<Option<Payment>, DbError> {
        let data = self.data.lock().unwrap();
        Ok(data
            .iter()
            .find(|payment| payment.payment_id == payment_id)
            .cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db() {
        let payment = Payment {
            payment_id: "1".to_string(),
            circ_id: "1".to_string(),
            round: 1,
            relay_fingerprint: "1".to_string(),
            updated_at: 1,
            amount_msat: 1,
        };

        let db = Db::new("test.json".to_string()).unwrap();
        db.write_payment(payment).unwrap();

        let payment1 = db.lookup_payment("1".to_string()).unwrap();
        assert_eq!(payment1.unwrap().payment_id, "1".to_string());
    }
}
