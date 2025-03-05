use super::types::InfoResponse;
use crate::types::NodeInfo;
use crate::ApiError;
use reqwest::header;

// https://docs.corelightning.org/reference/get_list_methods_resource

pub fn get_info(base: String, rune: String) -> Result<NodeInfo, ApiError> {
    let req_url = format!("{}/v1/getinfo", base);
    println!("Constructed URL: {} rune {}", req_url, rune);
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Rune", header::HeaderValue::from_str(&rune).unwrap());
    let client = reqwest::blocking::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .default_headers(headers.clone())
        .build()
        .unwrap();

    let response = client.post(&req_url).send().unwrap();
    let response_text = response.text().unwrap();
    println!("Raw response: {}", response_text);
    let info: InfoResponse = serde_json::from_str(&response_text)?;

    let node_info = NodeInfo {
        alias: info.alias,
        color: info.color,
        pubkey: info.id,
        network: info.network,
        block_height: info.blockheight,
        block_hash: "".to_string(),
    };
    Ok(node_info)
}
