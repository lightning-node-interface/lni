use lni::DbError;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use napi::bindgen_prelude::*;
use napi_derive::{napi};


#[napi]
#[derive(Debug, Deserialize)]
pub struct Db {
    path: String,
    #[serde(skip)]
    data: Arc<Mutex<Vec<lni::Payment>>>,
}

#[napi]
impl Db {
    #[napi(constructor)]
    pub fn new(path: String) -> Result<Self> {
        let data = if let Ok(mut file) = File::open(&path) {
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .map_err(|e| napi::Error::from_reason(format!("IoErr: {}", e.to_string())))?;
            serde_json::from_str(&contents).map_err(|e| DbError::DeserializationErr {
                reason: e.to_string(),
            }).unwrap()
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            data: Arc::new(Mutex::new(data)),
        })
    }

    #[napi]
    pub fn save(&self) -> Result<()> {
        let data = self.data.lock().unwrap();
        let json = serde_json::to_string_pretty(&*data).map_err(|e| DbError::SerializationErr {
            reason: e.to_string(),
        }).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|e| DbError::IoErr {
                reason: e.to_string(),
            }).unwrap();
        file.write_all(json.as_bytes())
            .map_err(|e| DbError::IoErr {
                reason: e.to_string(),
            }).unwrap();
        Ok(())
    }

    #[napi]
    pub fn write_payment(&self, payment: lni::Payment) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        data.push(payment);
        drop(data); // Explicitly drop the lock before saving
        self.save()
    }
 
    #[napi]
    pub fn lookup_payment(&self, payment_id: String) -> Result<Option<lni::Payment>> {
        let data = self.data.lock().unwrap();
        Ok(data
            .iter()
            .find(|payment| payment.payment_id == payment_id)
            .cloned())
    }
}