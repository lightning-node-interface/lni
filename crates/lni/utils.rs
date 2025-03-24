use lightning_invoice::Bolt11Invoice;
use std::error::Error;
use std::str::FromStr;
use regex::Regex;

pub fn calculate_fee_msats(
    bolt11: &str,
    fee_percentage: f64,
    amount_msats: Option<u64>,
) -> Result<u64, Box<dyn Error>> {
    // Decode the BOLT11 invoice
    let invoice = Bolt11Invoice::from_str(bolt11).unwrap();

    println!("invoice: {:?}", invoice);

    // Get the amount from the invoice
    let invoice_amount_msats = invoice.amount_milli_satoshis().unwrap_or(0);
    println!("invoice amount: {:?}", invoice_amount_msats);

    // Determine the amount to use for fee calculation
    let amount_msats = if invoice_amount_msats == 0 {
        amount_msats.ok_or("Amount in invoice is 0 and no amount_msats provided")?
    } else {
        invoice_amount_msats
    };

    // Calculate the fee
    let fee_msats = (amount_msats as f64 * fee_percentage / 100.0).round() as u64;

    println!(
        "calculated fee_limit_msat {} from percentage, {} for the total amount {}",
        fee_msats, fee_percentage, amount_msats
    );

    Ok(fee_msats)
}
