use reqwest::Client;
use serde_json::{json, Value};

pub enum SessionStatus {
    Active,
    Expired,
    Banned { reason: String, appeal: String },
    Unavailable(String),
}

#[derive(Clone)]
pub struct Connection {
    pub http: Client,
    pub api: String,
    pub token: String,
}

#[cfg(all(test, windows))]
mod credential_tests {
    #[test]
    fn windows_credential_round_trip_uses_a_separate_synthetic_entry() {
        let name = format!("fixture-{}", std::process::id());
        let entry = keyring::Entry::new("Selfnify Lume Code validation", &name).unwrap();
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let cleanup = Cleanup(entry);
        cleanup
            .0
            .set_password("synthetic-not-a-user-token")
            .unwrap();
        assert_eq!(
            cleanup.0.get_password().unwrap(),
            "synthetic-not-a-user-token"
        );
    }
}
pub fn api() -> Result<String, String> {
    let value = std::env::var("LUME_CODE_API")
        .unwrap_or_else(|_| "https://api.lume.dpdns.org".into())
        .trim_end_matches('/')
        .to_owned();
    let url = reqwest::Url::parse(&value).map_err(|e| e.to_string())?;
    if url.scheme() != "https"
        && !(url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost")))
    {
        return Err("The backend must use HTTPS (or localhost for development).".into());
    }
    if url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Use a plain backend origin.".into());
    }
    Ok(value)
}
fn credential(api: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("Selfnify Lume Code", api).map_err(|e| e.to_string())
}
impl Connection {
    pub fn load() -> Result<Self, String> {
        let api = api()?;
        let token = credential(&api)?
            .get_password()
            .map_err(|_| "Sign in first: lume-code login.".to_owned())?;
        Ok(Self {
            http: Client::builder()
                .user_agent("LumeCode/0.1.0")
                .connect_timeout(std::time::Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?,
            api,
            token,
        })
    }
    pub async fn request(&self, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let req = if let Some(b) = body {
            self.http.post(format!("{}{path}", self.api)).json(b)
        } else {
            self.http.get(format!("{}{path}", self.api))
        };
        let response = req
            .bearer_auth(&self.token)
            .timeout(std::time::Duration::from_secs(25))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .map_err(|_| "The backend returned an unreadable response.")?;
        if !status.is_success() {
            return Err(value["error"]
                .as_str()
                .unwrap_or("The request failed.")
                .into());
        }
        Ok(value)
    }

    pub async fn check_session(&self) -> SessionStatus {
        let response = match self
            .http
            .get(format!("{}/auth/me", self.api))
            .bearer_auth(&self.token)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => {
                return SessionStatus::Unavailable(
                    "Lume could not be reached to check this account.".into(),
                )
            }
        };
        let status = response.status();
        let value: Value = match response.json().await {
            Ok(value) => value,
            Err(_) => {
                return SessionStatus::Unavailable(
                    "Lume returned an unreadable account status.".into(),
                )
            }
        };
        if status.is_success() {
            SessionStatus::Active
        } else if value["code"] == "banned" {
            SessionStatus::Banned {
                reason: value["reason"].as_str().unwrap_or("").trim().to_owned(),
                appeal: value["appeal"].as_str().unwrap_or("").trim().to_owned(),
            }
        } else if status == reqwest::StatusCode::UNAUTHORIZED {
            SessionStatus::Expired
        } else {
            SessionStatus::Unavailable(
                value["error"]
                    .as_str()
                    .unwrap_or("Lume could not check this account.")
                    .to_owned(),
            )
        }
    }
}

pub fn forget(api: &str) {
    if let Ok(entry) = credential(api) {
        let _ = entry.delete_credential();
    }
}
/// Only the private session returned after browser approval is persisted.
pub fn store(api: &str, token: &str) -> Result<(), String> {
    if token.len() != 68 || !token.starts_with("lms_") {
        return Err("Invalid account session.".into());
    }
    credential(api)?
        .set_password(token)
        .map_err(|e| e.to_string())
}
pub async fn command(args: &[String]) -> Result<bool, String> {
    let Some(action) = args.get(1).map(String::as_str) else {
        return Ok(false);
    };
    if action == "--help" || action == "help" {
        println!("Lume Code\n  lume-code [--project PATH]\n  lume-code login\n  lume-code logout\n\nSign-in opens your existing Lume login page. The terminal also shows a copyable URL.\nType / for commands; ? for shortcuts. /resume opens saved sessions. /dirs shows folder access. /side-history shows side answers. Ctrl-T expands latest inline tools; Ctrl-R prepares recovery. Shift-Tab changes Plan/Edit. Escape stops.");
        return Ok(true);
    }
    if action != "logout" {
        return Ok(false);
    }
    let api = api()?;
    if let Ok(c) = Connection::load() {
        if c.request("/auth/logout", Some(&json!({}))).await.is_err() {
            eprintln!("Could not confirm server sign-out. Revoke this session in Lume Settings if needed.");
        }
    }
    let _ = credential(&api)?.delete_credential();
    println!("Signed out on this PC.");
    Ok(true)
}
