use serde_json::Value;
use std::time::Duration;

pub struct ApiClient {
    base_url: String,
    api_key: Option<String>,
    client: reqwest::blocking::Client,
}

impl ApiClient {
    pub fn new(address: &str, api_key: Option<&str>) -> Self {
        let base_url = address.trim_end_matches('/').to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        Self {
            base_url,
            api_key: api_key.map(String::from),
            client,
        }
    }

    pub fn get(&self, path: &str) -> Result<Value, String> {
        self.request("GET", path, None::<&Value>)
    }

    pub fn get_with_query(&self, path: &str, params: &[(&str, &str)]) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.get(&url);
        for (k, v) in params {
            req = req.query(&[(k, v)]);
        }
        if let Some(key) = &self.api_key {
            req = req.header("X-API-Key", key);
        }
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        let status = resp.status();
        let body: Value = resp.json().map_err(|e| format!("Failed to parse response: {}", e))?;
        if status.is_success() {
            Ok(body)
        } else {
            let msg = body
                .get("message")
                .or(body.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            Err(format!("{} (HTTP {})", msg, status.as_u16()))
        }
    }

    pub fn post(&self, path: &str, body: Option<&Value>) -> Result<Value, String> {
        self.request("POST", path, body)
    }

    pub fn put(&self, path: &str, body: Option<&Value>) -> Result<Value, String> {
        self.request("PUT", path, body)
    }

    pub fn delete(&self, path: &str) -> Result<Value, String> {
        self.request("DELETE", path, None::<&Value>)
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let req = match method {
            "GET" => self.client.get(&url),
            "POST" => {
                let mut r = self.client.post(&url);
                if let Some(b) = body {
                    r = r.json(b);
                }
                r
            }
            "PUT" => {
                let mut r = self.client.put(&url);
                if let Some(b) = body {
                    r = r.json(b);
                }
                r
            }
            "DELETE" => self.client.delete(&url),
            _ => return Err(format!("Unsupported method: {}", method)),
        };
        let req = if let Some(key) = &self.api_key {
            req.header("X-API-Key", key)
        } else {
            req
        };
        let resp = req.send().map_err(|e| format!("Request failed: {}", e))?;
        let status = resp.status();
        let body: Value = resp.json().map_err(|e| format!("Failed to parse response: {}", e))?;
        if status.is_success() {
            Ok(body)
        } else {
            let msg = body
                .get("message")
                .or(body.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            Err(format!("{} (HTTP {})", msg, status.as_u16()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_client_defaults() {
        let client = ApiClient::new("http://127.0.0.1:8642", None);
        assert_eq!(client.base_url, "http://127.0.0.1:8642");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn test_new_client_with_api_key() {
        let client = ApiClient::new("http://localhost:9999", Some("test-key"));
        assert_eq!(client.base_url, "http://localhost:9999");
        assert_eq!(client.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_new_client_strips_trailing_slash() {
        let client = ApiClient::new("http://localhost:8642/", None);
        assert_eq!(client.base_url, "http://localhost:8642");
    }

    #[test]
    fn test_request_connection_refused() {
        let client = ApiClient::new("http://127.0.0.1:1", None);
        let result = client.get("/health");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Request failed"));
    }
}
