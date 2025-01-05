use lni::{
    ILightningNode, 
    WalletInterface,
    lnd::LndNode, 
    LndConfig, 
    cln::ClnNode,
    ClnConfig,
};
pub fn main() {

    // LND
    let lnd_config: LndConfig = LndConfig {
        macaroon: "0201036c6e6420[...]".to_string(),
        url: "https://127.0.0.1:8080".to_string(),
        wallet_interface: WalletInterface::LND_REST,
    };
    let lnd_node =  LndNode::new(lnd_config);
    let lnd_result =  lnd_node.pay_invoice("invoice").unwrap();
    println!("Pay LND invoice result {}", lnd_result);


     // CLN
     let cln_config: ClnConfig = ClnConfig {
        rune: "0201036c6e6420[...]".to_string(),
        url: "https://127.0.0.1:8081".to_string(),
        wallet_interface: WalletInterface::CLN_REST,
    };
    let cln_node =  ClnNode::new(cln_config);
    let cln_result =  cln_node.pay_invoice("invoice").unwrap();
    println!("Pay CLN invoice result {}", cln_result);

}