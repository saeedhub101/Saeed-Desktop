use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CharacterProfile {
    pub schema_version: u32,
    #[serde(default)] pub bone_map: serde_json::Map<String, serde_json::Value>,
    #[serde(default)] pub rest_pose: serde_json::Map<String, serde_json::Value>,
    #[serde(default)] pub axis_map: serde_json::Map<String, serde_json::Value>,
    #[serde(default)] pub limits: serde_json::Map<String, serde_json::Value>,
    #[serde(default)] pub motion_tune: serde_json::Map<String, serde_json::Value>,
    #[serde(default="default_scale")] pub scale: f32,
}
fn default_scale()->f32{1.0}

impl Default for CharacterProfile {
    fn default()->Self {
        Self {
            schema_version:1,
            bone_map:serde_json::Map::new(),
            rest_pose:serde_json::Map::new(),
            axis_map:serde_json::Map::new(),
            limits:serde_json::Map::new(),
            motion_tune:serde_json::Map::new(),
            scale:1.0,
        }
    }
}

pub fn profile_path(root:&Path, character_id:&str)->PathBuf {
    root.join("characters").join(character_id).join("profile.json")
}

pub fn load(root:&Path, character_id:&str)->Result<CharacterProfile,String> {
    let path=profile_path(root,character_id);
    if !path.exists(){ return Ok(CharacterProfile::default()); }
    let text=fs::read_to_string(path).map_err(|e|e.to_string())?;
    let mut profile:CharacterProfile=serde_json::from_str(&text).map_err(|e|e.to_string())?;
    if profile.schema_version == 0 { profile.schema_version=1; }
    Ok(profile)
}

pub fn save(root:&Path, character_id:&str, profile:&CharacterProfile)->Result<(),String> {
    let dir=root.join("characters").join(character_id);
    fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
    let tmp=dir.join("profile.json.tmp");
    let path=dir.join("profile.json");
    fs::write(&tmp,serde_json::to_vec_pretty(profile).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    fs::OpenOptions::new().read(true).write(true).open(&tmp).map_err(|e|e.to_string())?.sync_all().map_err(|e|e.to_string())?;
    fs::rename(&tmp,&path).map_err(|e|e.to_string())
}
