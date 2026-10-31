use crate::{connection::Connection, local};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
pub enum Event {
    History(Vec<Value>),
    Text(String),
    Activity(Value),
    Approval {
        label: String,
        reply: tokio::sync::oneshot::Sender<bool>,
    },
    Notice(String),
    Done,
    Directory(String),
    State(Value),
    Picker(Vec<Value>),
    Usage(Value),
    UsageRefresh,
}
pub struct Turn {
    pub text: String,
    pub plan: bool,
}
pub struct Agent {
    pub send: mpsc::Sender<Turn>,
    pub events: mpsc::Receiver<Event>,
    stop: Arc<AtomicBool>,
}
impl Agent {
    pub fn new(root: PathBuf) -> Self {
        Self::spawn(root, None)
    }
    fn spawn(root: PathBuf, provided: Option<Connection>) -> Self {
        Self::spawn_at(root, provided, None)
    }
    fn spawn_at(
        root: PathBuf,
        provided: Option<Connection>,
        initial_path: Option<PathBuf>,
    ) -> Self {
        let (tx, input) = mpsc::channel::<Turn>();
        let (output, events) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let cancel = stop.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let mut history = vec![];
            let mut client = None;
            let mut images = Vec::new();
            let mut checkpoint = Value::Null;
            let mut saved_path = initial_path;
            let mut account_epoch = Value::Null;
            let mut account_email = String::new();
            let mut workspace = crate::session_commands::Settings::new(root);
            while let Ok(turn) = input.recv() {
                cancel.store(false, Ordering::Release);
                let result=rt.block_on(async{
  let root=workspace.root.clone();
  let connection=match &provided{Some(c)=>c.clone(),None=>Connection::load()?};
  if provided.is_none(){let identity=connection.request("/account/profile",None).await?;account_email=identity["email"].as_str().unwrap_or("").to_owned();let path=crate::sessions::location(&connection.api,identity["email"].as_str().unwrap_or(""),&root)?;if saved_path.as_ref()!=Some(&path){let lock=crate::session_store::Lock::acquire(&path)?;let mut saved=crate::sessions::load(&path)?;if !saved.is_null()&&saved["epoch"]!=identity["epoch"]{saved=Value::Null;}workspace=crate::session_commands::Settings::new(root.clone());workspace.lock=Some(lock);workspace.restore(&saved["settings"]);history=saved["messages"].as_array().cloned().unwrap_or_default();checkpoint=saved["checkpoint"].clone();client=saved["binding"].as_object().map(|_|saved["binding"].clone());if let Some(old)=&client{if connection.request(&format!("/code/clients/{}",old["id"].as_str().unwrap_or("")),None).await.is_err(){client=None;}}crate::session_store::restore(&path,&identity["epoch"],&mut history)?;crate::session_catalog::register(&connection.api,&account_email,&root)?;saved_path=Some(path);let _=output.send(Event::History(history.clone()));}
  account_epoch=identity["epoch"].clone();
  if client.as_ref().is_some_and(|b|b["epoch"]!=identity["epoch"]){history.clear();client=None;images.clear();checkpoint=Value::Null;let _=output.send(Event::History(vec![]));let _=output.send(Event::Notice("Your account data changed. A new conversation has been started.".into()));}}
  if turn.text=="/__restore"{return Ok(());}
  if crate::session_commands::run(&turn.text,&mut workspace,&connection,&account_email,&mut saved_path,&mut history,&mut client,&mut checkpoint,&account_epoch,&output)?{if matches!(turn.text.split_whitespace().next(),Some("/cd"|"/clear"|"/new"|"/resume")){images.clear();}return Ok(());}
  if turn.text=="/usage"{let v=connection.request("/account/usage",None).await?;for(key,label)in[("five_hour","Five hours"),("weekly","Weekly")]{let w=&v[key];let reset=chrono::DateTime::from_timestamp(w["resets_at"].as_i64().unwrap_or(0),0).map(|v|v.with_timezone(&chrono::Local).format("%a %H:%M").to_string()).unwrap_or_default();let _=output.send(Event::Text(format!("{label}: {} / {} messages used. Resets {reset}.\n",w["used"],w["limit"])));}return Ok(());}
  if let Some(old)=&client{if connection.request(&format!("/code/clients/{}",old["id"].as_str().unwrap_or("")),None).await.is_err(){client=None;}}
  if client.is_none(){client=Some(connection.request("/code/connect",Some(&json!({"root":root.display().to_string()}))).await?);}
  let binding=client.as_ref().unwrap();let id=binding["id"].as_str().ok_or("Reconnect this project.")?.to_owned();
  if crate::actions::run(&connection,binding,&root,&turn.text,&mut images,&output,cancel.clone()).await?{return Ok(());}
  let deep=turn.text.starts_with("/deep ");let text=turn.text.strip_prefix("/deep ").unwrap_or(&turn.text);
  let resume_context=if turn.text==crate::session_store::CONTINUE{saved_path.as_ref().map(|p|crate::session_store::resume_request(&crate::session_store::journal(p,&account_epoch)))}else{None};
  let text=resume_context.as_deref().unwrap_or(text);
  let compact=turn.text=="/compact";
  if !compact{history.push(json!({"role":"user","content":text,"image_files":std::mem::take(&mut images)}));}
  if compact&&history.is_empty(){let _=output.send(Event::Text("There is no earlier context to summarize yet.".into()));return Ok(());}
  let previous_run=saved_path.as_ref().map(|p|crate::session_store::journal(p,&account_epoch)).unwrap_or(Value::Null);
  let run_id=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos().to_string();
  let mut run=json!({"id":run_id,"epoch":account_epoch,"request":turn.text,"status":"running","tracked":!compact,"started_at":crate::session_store::now(),"text":"","activities":[]});
  let mut journal_tick=std::time::Instant::now();
  if let Some(path)=&saved_path{crate::sessions::save(path,&history,&client,&checkpoint,&account_epoch)?;crate::session_store::save_run(path,&run)?;}
  let wire=history.iter().map(|v|json!({"role":v["role"],"content":v["content"],"image_files":v["image_files"]})).collect::<Vec<_>>();
  let mut options=workspace.info();options["compact_only"]=json!(compact);
  let timezone=iana_time_zone::get_timezone().ok().filter(|v| !v.is_empty() && v.len()<=80).unwrap_or_else(||"UTC".into());
  let body=json!({"messages":wire,"compaction":checkpoint,"code_options":options,"deep_search":deep,"reasoning":workspace.effort,"chat_id":id,"data_epoch":binding["epoch"],"interaction_mode":if turn.plan{"plan"}else{"edit"},"tools":["web_search","astra_search","astra_preview_replace"],"timezone":timezone});
  let(response,poller)={let c=connection.clone();let root=root.clone();let directories=workspace.directories.clone();let output=output.clone();let polling_cancel=cancel.clone();let key=id.clone();let plan=turn.plan;let poller=tokio::spawn(async move{let cancel=polling_cancel;let mut saved:Option<(String,Value)>=None;while !cancel.load(Ordering::Acquire){
   if let Ok(v)=c.request(&format!("/code/clients/{key}"),None).await{let task=&v["task"];if let Some(task_id)=task["id"].as_str(){let mut result=if let Some((old,result))=saved.as_ref().filter(|(old,_)|old==task_id){let _=old;result.clone()}else{
    let name=task["name"].as_str().unwrap_or("");let args=&task["arguments"];let read_only=matches!(name,"read_file"|"list_files"|"search_files"|"get_info"|"astra_search"|"astra_preview_replace");
    let approved=if plan&&!read_only{false}else if local::needs_approval(name,args){let(tx,rx)=tokio::sync::oneshot::channel();let _=output.send(Event::Approval{label:format!("Approve {name} in {}?\n{}",local::working_root(&root,&directories,args).unwrap_or(&root).display(),if name=="run_command"{args["command"].as_str().unwrap_or("").to_owned()}else{args.to_string()}),reply:tx});tokio::select!{v=rx=>v.unwrap_or(false),_=cancelled(cancel.clone())=>false}}else{true};
    if approved&&!cancel.load(Ordering::Acquire){if c.request(&format!("/code/clients/{key}"),None).await.ok().is_some_and(|v|v["task"]["id"].as_str()==Some(task_id)){local::execute_in(&root,&directories,name,args).await}else{json!({"error":"This action expired or was cancelled. It was not run. Inspect the saved state before retrying."})}}else{json!({"error":"The user denied or stopped this action. Do not retry it or attempt a workaround."})}
   };if result.to_string().len()>200000{result=json!({"error":"The tool output exceeded the display limit. The action may have completed; inspect saved files before retrying."});}saved=Some((task_id.to_owned(),result.clone()));let _=c.request(&format!("/code/clients/{key}"),Some(&json!({"id":task_id,"result":result}))).await;
   }}tokio::time::sleep(Duration::from_millis(350)).await;
  }});let request=tokio::select!{r=tokio::time::timeout(Duration::from_secs(60),connection.http.post(format!("{}/code/chat",connection.api)).bearer_auth(&connection.token).header("x-lume-code-client",&id).json(&body).send())=>r.map_err(|_|"The backend did not respond. Try again shortly.".to_owned()).and_then(|r|r.map_err(|e|e.to_string())),_=cancelled(cancel.clone())=>Err("Stopped.".into())};(request,poller)};
  let checkpoint_before=checkpoint.clone();
  let outcome=async{let response=response.map_err(|e|e.to_string())?;if !response.status().is_success(){let status=response.status();let v:Value=response.json().await.unwrap_or_default();if let Some(usage)=v.get("usage"){let _=output.send(Event::Usage(usage.clone()));}if status.is_client_error(){run["status"]=json!("rejected");}return Err(v["error"].as_str().unwrap_or("Could not start this response.").to_owned());}let _=output.send(Event::UsageRefresh);let mut stream=response.bytes_stream();let mut buffer=vec![];let mut text=String::new();let mut trace=Vec::new();let mut done=false;let mut reported=false;
   loop{tokio::select!{_=cancelled(cancel.clone())=>{let _=output.send(Event::Notice("Stopped. Completed local changes are saved.".into()));break;},chunk=stream.next()=>{let Some(chunk)=chunk else{break};let chunk=match chunk{Ok(v)=>v,Err(_)=>{reported=true;let _=output.send(Event::Notice("The connection paused. Completed changes are saved. Send Continue when ready.".into()));break;}};buffer.extend_from_slice(&chunk);if buffer.windows(6).any(|window|window==b"[DONE]"){done=true;}if buffer.len()>1024*1024{return Err("The stream returned an oversized event.".into());}while let Some(end)=buffer.iter().position(|b|*b==b'\n'){let line=buffer.drain(..=end).collect::<Vec<_>>();let line=String::from_utf8_lossy(&line);let Some(data)=line.trim().strip_prefix("data:").map(str::trim)else{continue};if data=="[DONE]"{done=true;continue;}if let Ok(value)=serde_json::from_str::<Value>(data){if let Some(s)=value["choices"][0]["delta"]["content"].as_str(){text.push_str(s);let _=output.send(Event::Text(s.into()));}if value["lume"]["state"]=="context_checkpoint"{checkpoint=value["lume"]["checkpoint"].clone();}if value["lume"].is_object(){text.push_str(&crate::chat::tasker::card_note(&value["lume"]));let mut record=crate::chat::tasker::record(&value["lume"]);record["at"]=json!(text.len());record["time"]=json!(crate::session_store::now());trace.push(record);let _=output.send(Event::Activity(value["lume"].clone()));}if journal_tick.elapsed()>=Duration::from_millis(500)||matches!(value["lume"]["state"].as_str(),Some("tool_done"|"tool_execution")){run["text"]=json!(text);run["activities"]=json!(trace);if let Some(path)=&saved_path{crate::session_store::save_run(path,&run)?;}journal_tick=std::time::Instant::now();}if value["lume"]["state"]=="paused"||value["choices"][0]["finish_reason"]=="length"{reported=true;let _=output.send(Event::Notice("The response paused. Your saved progress can be continued.".into()));}if value["error"].is_object(){reported=true;let _=output.send(Event::Notice("The response paused. Completed changes are saved. Send Continue when ready.".into()));}}}}}}
   if !done && String::from_utf8_lossy(&buffer).contains("[DONE]"){done=true;}
   run["status"]=json!(if done&&!reported&&!cancel.load(Ordering::Acquire){"complete"}else{"interrupted"});run["text"]=json!(text);run["activities"]=json!(trace);if let Some(path)=&saved_path{crate::session_store::save_run(path,&run)?;}
   if !text.is_empty()||!trace.is_empty(){history.push(json!({"role":"assistant","content":text,"activities":trace,"run_id":run_id}));}if !done&&!reported&&!cancel.load(Ordering::Acquire){let _=output.send(Event::Notice("The connection ended early. Completed changes are saved. Send Continue when ready.".into()));}Ok::<(),String>(())
  }.await;poller.abort();if run["status"]=="rejected"&&crate::session_store::recovery_text(&previous_run).is_some(){run=previous_run;}if run["status"]=="running"{run["status"]=json!("interrupted");}if let Some(path)=&saved_path{crate::session_store::save_run(path,&run)?;}if compact&&outcome.is_ok(){let _=output.send(Event::Text(if checkpoint!=checkpoint_before{"Earlier context was summarized. Your full conversation stays on disk."}else{"No context was replaced. There may be too little earlier history, or summarizing is temporarily unavailable."}.into()));}outcome
 });
                if let Some(path) = &saved_path {
                    if let Err(e) =
                        crate::sessions::save(path, &history, &client, &checkpoint, &account_epoch)
                            .and_then(|_| {
                                crate::sessions::save_options(path, &workspace.persisted())
                            })
                    {
                        let _ = output.send(Event::Notice(e));
                    }
                }
                if let Some(path) = &saved_path {
                    if provided.is_none() {
                        let _ = crate::session_catalog::register(
                            &crate::connection::api().unwrap_or_default(),
                            &account_email,
                            &workspace.root,
                        );
                    }
                    let _ = output.send(Event::State(crate::session_store::snapshot(
                        path,
                        &account_epoch,
                        &workspace.root,
                        &workspace.directories,
                    )));
                }
                if let Err(e) = result {
                    let _ = output.send(Event::Notice(e));
                }
                let _ = output.send(Event::Done);
            }
        });
        Self {
            send: tx,
            events,
            stop,
        }
    }
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
    }
}
async fn cancelled(stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
