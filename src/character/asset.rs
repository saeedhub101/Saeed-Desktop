use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterAssetReport {
    pub path: PathBuf,
    pub scenes: usize,
    pub meshes: usize,
    pub skins: usize,
    pub animations: usize,
    pub bones: Vec<String>,
}

pub fn inspect_gltf(path: impl AsRef<Path>) -> Result<CharacterAssetReport, String> {
    let path = path.as_ref().to_path_buf();
    let (document, _, _) = gltf::import(&path)
        .map_err(|error| format!("Could not load character asset '{}': {error}", path.display()))?;

    let mut bones = Vec::new();
    for skin in document.skins() {
        for joint in skin.joints() {
            if let Some(name) = joint.name() {
                bones.push(name.to_string());
            }
        }
    }

    Ok(CharacterAssetReport {
        path,
        scenes: document.scenes().count(),
        meshes: document.meshes().count(),
        skins: document.skins().count(),
        animations: document.animations().count(),
        bones,
    })
}

pub fn default_asset_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(base) = std::env::var_os("APPDATA") {
        let base = PathBuf::from(base).join("Saeed");
        candidates.push(base.join("Saeed_AI-3D.glb"));
        candidates.push(base.join("Saeed.glb"));
    }

    candidates.push(PathBuf::from("assets/Saeed_AI-3D.glb"));
    candidates.push(PathBuf::from("assets/Saeed.glb"));
    candidates
}

pub fn find_default_asset() -> Option<PathBuf> {
    default_asset_candidates().into_iter().find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::default_asset_candidates;

    #[test]
    fn asset_candidates_are_deterministic() {
        let candidates = default_asset_candidates();
        assert!(candidates.len() >= 2);
    }
}
