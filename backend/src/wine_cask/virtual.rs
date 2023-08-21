use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::steam_util::CompatibilityTool;
use crate::wine_cask::generate_compatibility_tool_vdf;
use crate::wine_cask::wine_cask::WineCask;



// Internal only
#[derive(Serialize, Deserialize, Clone)]
pub struct VirtualCompatibilityToolMetadata {
    r#virtual: bool,
    virtual_original: String,
}


impl WineCask {
    fn lookup_virtual_compatibility_tool_metadata(&self, compat_tool: &CompatibilityTool) -> VirtualCompatibilityToolMetadata {
        let metadata_file = compat_tool.path.join("wine-cask-metadata.json"); // fixme: Store in runtime data dir instead
        if metadata_file.exists() && metadata_file.is_file() {
            let metadata = fs::read_to_string(metadata_file).unwrap();
            let metadata: VirtualCompatibilityToolMetadata =
                serde_json::from_str(&metadata).unwrap();
            metadata
        } else {
            VirtualCompatibilityToolMetadata {
                r#virtual: false,
                virtual_original: "".to_string(),
            }
        }
    }

    fn create_virtual_compatibility_tool(&self, name: &str, virtual_original_path: PathBuf) {
        let path = self
            .steam_util
            .get_steam_compatibility_tools_directory()
            .join(name);
        if path.exists() {
            // todo: already exist
        }

        fs::create_dir(&path).expect("TODO: panic message");
        fs::copy(virtual_original_path, &path).expect("TODO: panic message");

        // Generate virtual compat tool vdf
        let compat_tool_vdf_path = path.join("compatibilitytool.vdf");
        let virtual_original = self
            .steam_util
            .read_compatibility_tool_from_vdf_path(&compat_tool_vdf_path)
            .unwrap()
            .display_name;
        generate_compatibility_tool_vdf(compat_tool_vdf_path, &name.replace(' ', "-"), name);

        // Create virtual compat tool metadata
        let metadata_file = path.join("wine-cask-metadata.json");
        let metadata = VirtualCompatibilityToolMetadata {
            r#virtual: true,
            virtual_original,
        };
        fs::write(
            metadata_file,
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
            .unwrap();
    }
}