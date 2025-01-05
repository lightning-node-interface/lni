use lni::{lnd::LndNode, ILightningNode};

pub fn main() {
    let node = LndNode;
    let result =  node.pay_invoice("invoice").unwrap();
    print!("Pay invoice result {}", result);

}