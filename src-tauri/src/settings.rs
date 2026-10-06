use std::{fs,path::Path};use serde::{Deserialize,Serialize};
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]pub struct AppSettings{pub schema_version:u32,pub character:CharacterSettings,pub performance:PerformanceSettings}
#[derive(Debug,Clone,Serialize,Deserialize)]#[serde(rename_all="camelCase")]pub struct CharacterSettings{pub visible:bool,pub scale:CharacterScale,pub position:Position,pub always_on_top:bool,pub current_id:String}
#[derive(Debug,Clone,Serialize,Deserialize)]pub struct Position{pub x:i32,pub y:i32}
#[derive(Debug,Clone,Copy,Serialize,Deserialize)]#[serde(rename_all="lowercase")]pub enum CharacterScale{Small,Medium,Large}
#[derive(Debug,Clone,Serialize,Deserialize)]pub struct PerformanceSettings{pub low_power:bool}
impl Default for AppSettings{fn default()->Self{Self{schema_version:1,character:CharacterSettings{visible:true,scale:CharacterScale::Medium,position:Position{x:-1,y:-1},always_on_top:true,current_id:"default".into()},performance:PerformanceSettings{low_power:false}}}}
impl AppSettings{
pub fn load(dir:&Path)->Result<Self,String>{let p=dir.join("settings.json");if !p.exists(){return Ok(Self::default())}serde_json::from_str(&fs::read_to_string(p).map_err(|e|e.to_string())?).map_err(|e|e.to_string())}
pub fn save(&self,dir:&Path)->Result<(),String>{fs::create_dir_all(dir).map_err(|e|e.to_string())?;let p=dir.join("settings.json");let t=dir.join("settings.json.tmp");fs::write(&t,serde_json::to_vec_pretty(self).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;fs::OpenOptions::new().read(true).open(&t).map_err(|e|e.to_string())?.sync_all().map_err(|e|e.to_string())?;if p.exists(){let _=fs::remove_file(&p);}fs::rename(t,p).map_err(|e|e.to_string())}
}