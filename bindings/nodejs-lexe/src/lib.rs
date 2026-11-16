pub use lni::types::*;

// Both Node packages use the same wallet and Connect implementations.
#[path = "../../lni_nodejs/src/lexe.rs"]
mod lexe;
pub use lexe::LexeNode;

#[path = "../../lni_nodejs/src/lexe_connect.rs"]
mod lexe_connect;
pub use lexe_connect::{LexeConnectOptions, LexeConnectResponse, LexeConnectSession};
