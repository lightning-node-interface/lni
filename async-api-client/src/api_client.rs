/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::{run_task, ApiError, Result, TaskRunner};
use std::{collections::HashMap, sync::Arc};
use serde_json::from_str;


#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    async fn fetch(&self, url: String, credentials: String) -> Result<String>;
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json {
            reason: e.to_string(),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct Issue {
    pub url: String,
    pub title: String,
    pub state: IssueState,
}

#[derive(Debug, serde::Deserialize)]
pub struct Ip {
    pub origin: String,
}

#[derive(Debug, serde::Deserialize)]
pub enum IssueState {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "closed")]
    Closed,
}

pub struct ApiClient {
    http_client: Arc<dyn HttpClient>,
    task_runner: Arc<dyn TaskRunner>,
}

impl ApiClient {
    // Pretend this is a blocking call that needs to load the credentials from disk/network
    fn load_credentials_sync(&self) -> String {
        String::from("username:password")
    }

    async fn load_credentials(self: Arc<Self>) -> String {
        let self_cloned = Arc::clone(&self);
        run_task(&self.task_runner, move || {
            self_cloned.load_credentials_sync()
        })
        .await
    }
}

impl ApiClient {
    pub fn new(http_client: Arc<dyn HttpClient>, task_runner: Arc<dyn TaskRunner>) -> Self {
        Self {
            http_client,
            task_runner,
        }
    }

    pub async fn get_issue(
        self: Arc<Self>,
        owner: String,
        repository: String,
        issue_number: u32,
    ) -> Result<Issue> {
        let credentials = self.clone().load_credentials().await;
        let url =
            format!("https://api.github.com/repos/{owner}/{repository}/issues/{issue_number}");
        let body = self.http_client.fetch(url, credentials).await?;
        Ok(serde_json::from_str(&body)?)
    }

    pub async fn get_ip_address(self: Arc<Self>) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let response = client.get("https://httpbin.org/ip").send().unwrap();
        let resp = response.json::<HashMap<String, String>>().unwrap();
        let str = format!("{:#?}", resp);
        eprint!("ip str: {}", str);
        let converted_ip = serde_json::from_str(&str);
        match converted_ip {
            Ok(ip) => Ok(ip),
            Err(e) => {
                eprint!("error: {}", e);
                return Err(ApiError::Json {
                    reason: e.to_string(),
                });
            }
        }
    }
}

pub struct Fetcher {}
impl Fetcher {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn get_ip_address(self: Arc<Self>) -> Result<Ip> {
        let client = reqwest::blocking::Client::new();
        let response = client.get("https://httpbin.org/ip").send().unwrap();
        let resp_text = response.text().unwrap();
        let ip_address_response: Ip = from_str(&resp_text)?;
        Ok(ip_address_response)
    }
}
