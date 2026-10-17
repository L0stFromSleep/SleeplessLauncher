use crate::state::{
    ContentSet, Hooks, Instance, InstanceLink, MemorySettings, WindowSize,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A declarative patch applied to a single mod jar inside an instance's
/// `mods` folder right before launch. Lets an instance work around a mod
/// bug (or just customize its behavior) without hand-editing the jar every
/// time it's updated or reinstalled: the launcher reapplies the patch
/// automatically whenever the jar or the patch definition changes, and
/// restores from a kept original (`<jar>.orig`) each time so patches never
/// stack on top of each other.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModJarPatch {
    /// File name of the jar to patch, as it appears in the instance's
    /// `mods` folder (e.g. `"nightmarespell.jar"`).
    pub jar_file_name: String,
    /// Entries inside the jar to overwrite wholesale, keyed by their
    /// path within the jar (e.g.
    /// `"data/voidspell/worldgen/noise_settings/void_realm.json"`), with
    /// the value being an absolute path on disk to read the replacement
    /// bytes from.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub replace_entries: HashMap<String, PathBuf>,
    /// SpongePowered Mixin config files inside the jar, keyed by their
    /// path within the jar (e.g. `"voidspell.mixins.json"`), with the
    /// value being the list of mixin class names to strip out of every
    /// string array in that file (covers `"mixins"`, `"client"`,
    /// `"server"`, or any other array the mod happens to use) before the
    /// game loads it. A path listed here is ignored if it's also present
    /// in `replace_entries`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub remove_mixins: HashMap<String, Vec<String>>,
}

impl ModJarPatch {
    pub fn is_noop(&self) -> bool {
        self.replace_entries.is_empty() && self.remove_mixins.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceLaunchOverrides {
    pub instance_id: String,
    pub java_path: Option<String>,
    pub extra_launch_args: Option<Vec<String>>,
    pub custom_env_vars: Option<Vec<(String, String)>>,
    pub memory: Option<MemorySettings>,
    pub force_fullscreen: Option<bool>,
    pub game_resolution: Option<WindowSize>,
    pub hooks: Hooks,
    /// Jar patches applied to this instance's mods before every launch.
    /// See [`ModJarPatch`].
    pub jar_patches: Vec<ModJarPatch>,
    /// When set, a launch that wasn't explicitly asked to join a server
    /// (i.e. [`crate::instance::QuickPlayType::None`]) boots straight into
    /// this singleplayer world instead of stopping at the title screen.
    /// Useful for an instance whose default menu is a modded multiplayer
    /// front door you don't want to go through.
    pub quick_play_singleplayer_world: Option<String>,
}

impl InstanceLaunchOverrides {
    pub fn empty(instance_id: String) -> Self {
        Self {
            instance_id,
            java_path: None,
            extra_launch_args: None,
            custom_env_vars: None,
            memory: None,
            force_fullscreen: None,
            game_resolution: None,
            hooks: Hooks {
                pre_launch: None,
                wrapper: None,
                post_exit: None,
            },
            jar_patches: Vec::new(),
            quick_play_singleplayer_world: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct InstanceLaunchOverridesData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_launch_args: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_env_vars: Option<Vec<(String, String)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemorySettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_fullscreen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_resolution: Option<WindowSize>,
    #[serde(default)]
    pub hooks: Hooks,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jar_patches: Vec<ModJarPatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quick_play_singleplayer_world: Option<String>,
}

impl InstanceLaunchOverridesData {
    pub(crate) fn into_launch_overrides(
        self,
        instance_id: String,
    ) -> InstanceLaunchOverrides {
        InstanceLaunchOverrides {
            instance_id,
            java_path: self.java_path,
            extra_launch_args: self.extra_launch_args,
            custom_env_vars: self.custom_env_vars,
            memory: self.memory,
            force_fullscreen: self.force_fullscreen,
            game_resolution: self.game_resolution,
            hooks: self.hooks,
            jar_patches: self.jar_patches,
            quick_play_singleplayer_world: self.quick_play_singleplayer_world,
        }
    }
}

impl From<&InstanceLaunchOverrides> for InstanceLaunchOverridesData {
    fn from(overrides: &InstanceLaunchOverrides) -> Self {
        Self {
            java_path: overrides.java_path.clone(),
            extra_launch_args: overrides.extra_launch_args.clone(),
            custom_env_vars: overrides.custom_env_vars.clone(),
            memory: overrides.memory,
            force_fullscreen: overrides.force_fullscreen,
            game_resolution: overrides.game_resolution,
            hooks: overrides.hooks.clone(),
            jar_patches: overrides.jar_patches.clone(),
            quick_play_singleplayer_world: overrides
                .quick_play_singleplayer_world
                .clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceLaunchContext {
    pub instance: Instance,
    pub applied_content_set: ContentSet,
    pub link: InstanceLink,
    pub launch_overrides: InstanceLaunchOverrides,
}
