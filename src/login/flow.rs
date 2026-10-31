use serde_json::{json, Value};
#[cfg(test)]
#[path = "flow_tests.rs"]
mod network_tests;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
pub const WEBSITE: &str = "https://lume.dpdns.org";
#[derive(Clone)]
pub struct Link {
    pub token: String,
    pub url: String,
    pub code: String,
    pub expires_in: u64,
}
pub enum Event {
    Ready(Link),
    Status(String),
    Connected(String),
    Failed(String),
    Cancelled,
}
pub struct Flow {
    pub events: mpsc::Receiver<Event>,
    stop: Arc<AtomicBool>,
}
impl Flow {
    pub fn start(api: String) -> Self {
        let (tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        std::thread::spawn(move || {
            let result = tokio::runtime::Runtime::new()
                .map_err(|e| e.to_string())
                .and_then(|rt| {
                    rt.block_on(authenticate(&api, &tx, cancel, crate::connection::store))
                });
            let event = match result {
                Ok(Some(email)) => Event::Connected(email),
                Ok(None) => Event::Cancelled,
                Err(e) => Event::Failed(e),
            };
            let _ = tx.send(event);
        });
        Self { events, stop }
    }
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
    }
}
impl Drop for Flow {
    fn drop(&mut self) {
        self.cancel();
    }
}
async fn stopped(stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}
struct Response {
    status: reqwest::StatusCode,
    value: Value,
}
async fn post(
    http: &reqwest::Client,
    api: &str,
    path: &str,
    body: &Value,
) -> Result<Response, String> {
    let r = http
        .post(format!("{api}{path}"))
        .json(body)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| "Couldn't reach Lume. Check your connection and try again.")?;
    let status = r.status();
    let value = r
        .json()
        .await
        .map_err(|_| "Lume returned an unreadable response. Please try again.")?;
    Ok(Response { status, value })
}
fn request_error(v: &Value) -> String {
    v["error"]
        .as_str()
        .unwrap_or("This sign-in request could not be completed. Get a new link.")
        .to_owned()
}
fn link(value: &Value) -> Result<Link, String> {
    let token = value["public_token"].as_str().unwrap_or("");
    let proof = value["poll_secret"].as_str().unwrap_or("");
    let expires = value["expires_in"].as_u64().unwrap_or(0);
    if token.len() != 100
        || !token.bytes().all(|c| c.is_ascii_alphanumeric())
        || proof.len() != 68
        || !proof.starts_with("lms_")
        || !(1..=600).contains(&expires)
    {
        return Err("Lume returned an invalid sign-in link. Please try again.".into());
    }
    let url = format!("{WEBSITE}/login?code_token={token}");
    if value["verification_uri"].as_str() != Some(&url) {
        return Err("The sign-in address could not be verified.".into());
    }
    Ok(Link {
        token: token.into(),
        url,
        code: format!("{}-{}", &token[..4], &token[96..]),
        expires_in: expires,
    })
}
pub(super) async fn authenticate(
    api: &str,
    events: &mpsc::Sender<Event>,
    stop: Arc<AtomicBool>,
    store: impl Fn(&str, &str) -> Result<(), String>,
) -> Result<Option<String>, String> {
    let http = reqwest::Client::builder()
        .user_agent("LumeCode/0.1.0")
        .connect_timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let start_body = json!({});
    let started = tokio::select! {_=stopped(&stop)=>return Ok(None),r=post(&http,api,"/code/login/start",&start_body)=>r?};
    if !started.status.is_success() {
        return Err(request_error(&started.value));
    }
    let public = link(&started.value)?;
    let body = json!({"public_token":public.token,"poll_secret":started.value["poll_secret"]});
    let deadline = tokio::time::Instant::now() + Duration::from_secs(public.expires_in);
    let interval =
        Duration::from_secs(started.value["interval"].as_u64().unwrap_or(3).clamp(1, 10));
    let _ = events.send(Event::Ready(public));
    loop {
        let response = tokio::select! {_=stopped(&stop)=>{let _=post(&http,api,"/code/login/cancel",&body).await;return Ok(None)},r=post(&http,api,"/code/login/poll",&body)=>r};
        match response {
            Ok(r) if r.status.is_success() => match r.value["state"].as_str() {
                Some("approved") => {
                    let token = r.value["token"]
                        .as_str()
                        .filter(|s| s.len() == 68 && s.starts_with("lms_"))
                        .ok_or("Lume did not return a valid session.")?;
                    if stop.load(Ordering::Acquire) {
                        let _ = post(&http, api, "/code/login/cancel", &body).await;
                        return Ok(None);
                    }
                    let _ = events.send(Event::Status("Finishing sign-in…".into()));
                    if let Err(e) = store(api, token) {
                        let _ = post(&http, api, "/code/login/cancel", &body).await;
                        return Err(format!("Could not securely save your session: {e}"));
                    }
                    let _ = post(&http, api, "/code/login/ack", &body).await;
                    return Ok(Some(
                        r.value["user"]["email"]
                            .as_str()
                            .unwrap_or("your Lume account")
                            .into(),
                    ));
                }
                Some("pending") => {
                    let _ = events.send(Event::Status(
                        "Waiting for approval in your browser…".into(),
                    ));
                }
                Some("denied" | "cancelled") => {
                    return Err(
                        "Sign-in was cancelled. You can start a new sign-in when ready.".into(),
                    )
                }
                Some("consumed") => {
                    return Err("This link has already been used. Start a new sign-in.".into())
                }
                _ => return Err("This sign-in link is no longer available. Get a new link.".into()),
            },
            Ok(r)
                if r.status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || r.status.is_server_error() =>
            {
                let _ = events.send(Event::Status("Waiting for a connection to Lume…".into()));
            }
            Ok(r) => return Err(request_error(&r.value)),
            Err(_) => {
                let _ = events.send(Event::Status("Waiting for a connection to Lume…".into()));
            }
        }
        tokio::select! {_=stopped(&stop)=>{let _=post(&http,api,"/code/login/cancel",&body).await;return Ok(None)},_=tokio::time::sleep_until(deadline)=>return Err("Your sign-in link expired. Get a new link to try again.".into()),_=tokio::time::sleep(interval)=>{}}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_exact_public_link_is_displayable() {
        let token = "a".repeat(100);
        let mut v = json!({"public_token":token,"poll_secret":format!("lms_{}","b".repeat(64)),"verification_uri":format!("{WEBSITE}/login?code_token={token}"),"expires_in":600});
        let ready = link(&v).unwrap();
        assert_eq!(ready.token.len(), 100);
        assert!(!ready.url.contains("poll_secret"));
        v["verification_uri"] = json!("https://other.invalid/");
        assert!(link(&v).is_err());
    }
}
