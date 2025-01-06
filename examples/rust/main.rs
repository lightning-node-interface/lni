use lni::{
    lnd::LndNode, 
    cln::ClnNode,
};  
pub fn main() {

    // LND
    let lnd_node = LndNode::new("test_macaroon".to_string(), "https://127.0.0.1:8080".to_string());
    let lnd_result =  lnd_node.pay_invoice("invoice".to_string());
    println!("Pay LND invoice result {}", lnd_result);
    let lnd_txns = lnd_node.get_wallet_transactions("wallet_id".to_string());
    lnd_txns.iter().for_each(|txn| {
        println!("LND Transaction amount: {}, date: {}, memo: {}", txn.amount(), txn.date(), txn.memo()); 
    });
    let lnd_macaroon = lnd_node.key();

     // CLN
     let cln_node = ClnNode::new("test_rune".to_string(), "https://127.0.0.1:8081".to_string());
     let cln_result =  cln_node.pay_invoice("invoice".to_string());
     println!("Pay CLN invoice result {}", cln_result);
     let cln_txns = cln_node.get_wallet_transactions("wallet_id".to_string());
     cln_txns.iter().for_each(|txn| {
         println!("CLN Transaction amount: {}, date: {}, memo: {}", txn.amount(), txn.date(), txn.memo()); 
     });
     let cln_rune = cln_node.key();

}