//! Separate transport and context; a side question never writes main history.
use crate::connection::Connection;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
pub struct SideQuestion {
    pub events: mpsc::Receiver<Result<String, String>>,
    stop: Arc<AtomicBool>,
}
impl SideQuestion {
    pub fn start(root: String, question: String, mut context: Vec<Value>) -> Self {
        let (tx, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        std::thread::spawn(move || {
            let result=tokio::runtime::Runtime::new().map_err(|e|e.to_string()).and_then(|runtime|runtime.block_on(async{
                let c=Connection::load()?;
                let profile=c.request("/account/profile",None).await?;
                let binding=c.request("/code/connect",Some(&json!({"root":root}))).await?;
                context.push(json!({"role":"user","content":question}));
                let body=json!({"messages":context,"chat_id":binding["id"],"data_epoch":profile["epoch"],"interaction_mode":"plan","code_options":{"side_question":true}});
                let response=c.http.post(format!("{}/code/chat",c.api)).bearer_auth(&c.token).header("x-lume-code-client",binding["id"].as_str().unwrap_or("")).json(&body).timeout(std::time::Duration::from_secs(180)).send().await.map_err(|_|"The side question could not connect to Lume.")?;
                if !response.status().is_success(){let v:Value=response.json().await.unwrap_or_default();return Err(v["error"].as_str().unwrap_or("The side question could not be completed.").into());}
                let mut stream=response.bytes_stream();let mut buffer=Vec::new();let mut complete=false;let mut answered=false;
                loop {
                    let chunk=tokio::select!{r=stream.next()=>r,_=async{while !cancelled.load(Ordering::Acquire){tokio::time::sleep(std::time::Duration::from_millis(80)).await;}}=>return Err("Side question stopped.".into())};
                    let Some(chunk)=chunk else{break};buffer.extend_from_slice(&chunk.map_err(|_|"The side-question connection ended early.")?);
                    if buffer.len()>1024*1024{return Err("The side-question stream was too large.".into());}
                    while let Some(end)=buffer.iter().position(|b|*b==b'\n'){
                        let line=buffer.drain(..=end).collect::<Vec<_>>();let line=String::from_utf8_lossy(&line);
                        let Some(data)=line.trim().strip_prefix("data:").map(str::trim)else{continue};
                        if data=="[DONE]"{complete=true;continue;}
                        if let Ok(v)=serde_json::from_str::<Value>(data){
                            if let Some(text)=v["choices"][0]["delta"]["content"].as_str(){answered|=!text.is_empty();let _=tx.send(Ok(text.into()));}
                            if v["error"].is_object(){return Err("The side question paused. Please try again.".into());}
                        }
                    }
                }
                if !complete{return Err("The side-question connection ended early.".into());}
                if !answered{return Err("No answer came back for the side question. Please try again.".into());}
                Ok(())
            }));
            if let Err(error) = result {
                let _ = tx.send(Err(error));
            }
        });
        Self { events, stop }
    }
}
impl Drop for SideQuestion {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
