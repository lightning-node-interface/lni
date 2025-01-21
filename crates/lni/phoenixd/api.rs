pub async fn get_offer(url: String, password: String) -> Result<String, reqwest::Error> {
    let client: reqwest::blocking::Client = reqwest::blocking::Client::new();
    let req_url = format!("{}/getoffer", url);
    let response: reqwest::blocking::Response = client
        .get(&req_url)
        .basic_auth("", Some(password))
        .send()
        .unwrap();
    let offer_str = response.text().unwrap();
    Ok(offer_str)
}
