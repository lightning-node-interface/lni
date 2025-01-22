use std::sync::Arc;

pub struct Fetcher {
    url: String,
}
impl Fetcher {
    pub fn new(url: String) -> Self {
        Self { url }
    }

    pub fn get_config(&self) -> String {
        self.url.clone()
    }

    pub async fn get_ip_address(self: Arc<Self>) -> lni::Result<lni::Ip> {
        // match lni::get_ip_address().await {
        //     Ok(ip) => Ok(ip),
        //     Err(e) => Err(e),
        // }
        lni::get_ip_address().await
    }
}

// impl Default for Fetcher {
//     fn default(url: String) -> Self {
//         Self::new("https:")
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     async fn test_get_ip_address() {
//         let fetcher = Arc::new(Fetcher::new());
//         let result = fetcher.get_ip_address().await;

//         match result {
//             Ok(ip) => {
//                 println!("IP Address: {:?}", ip.origin);
//                 assert!(!ip.origin.is_empty());
//             }
//             Err(e) => {
//                 panic!("Failed to get IP address: {:?}", e);
//             }
//         }
//     }
// }