use crate::lightning_node_interface::LightningNodeInterface;
use async_trait::async_trait;

pub struct ClnNode {
    // Add any necessary fields here
}

impl ClnNode {
    // Constructor to create a new instance of ClnNode
    pub fn new() -> Self {
        ClnNode {
            // Initialize fields if necessary
        }
    }
}

// #[async_trait]
// impl LightningNodeInterface for ClnNode {
//     async fn create_invoice(&self, amount: u64, memo: String) -> String {
//         // Simulate asynchronous operation
//         async_std::task::sleep(std::time::Duration::from_secs(1)).await;
//         format!("{}:{}", amount, memo)
//     }
// }