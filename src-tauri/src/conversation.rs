use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id:String,
    pub title:String,
    pub created_at:i64,
    pub updated_at:i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id:i64,
    pub session_id:String,
    pub role:String,
    pub content:String,
    pub source:String,
    pub created_at:i64,
}

pub struct ConversationStore { path:PathBuf }
impl ConversationStore {
    pub fn open(root:&Path)->Result<Self,String>{
        let path=root.join("conversations.json");
        if !path.exists(){ fs::write(&path,b"[]").map_err(|e|e.to_string())?; }
        Ok(Self{path})
    }
    pub fn append(&self,message:&Message)->Result<(),String>{
        let mut all:Vec<Message>=serde_json::from_slice(&fs::read(&self.path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        all.push(message.clone());
        fs::write(&self.path,serde_json::to_vec_pretty(&all).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
    }
    pub fn messages(&self,session_id:&str)->Result<Vec<Message>,String>{
        let all:Vec<Message>=serde_json::from_slice(&fs::read(&self.path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        Ok(all.into_iter().filter(|m|m.session_id==session_id).collect())
    }
}
