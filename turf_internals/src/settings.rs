use std::path::PathBuf;

use serde::Deserialize;

use crate::{
    manifest::ManifestError,
    path_utils::{canonicalize, PathResolutionError},
};

#[derive(Deserialize, Debug, Default, Clone)]
pub struct FileOutput {
    pub(crate) global_css_file_path: Option<PathBuf>,
    pub(crate) separate_css_files_path: Option<PathBuf>,
}

pub(crate) static DEFAULT_CLASS_NAME_TEMPLATE: &str = "class-<id>";

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ClassNameGeneration {
    pub(crate) template: String,
    #[serde(default)]
    pub(crate) excludes: Vec<String>,
}

impl Default for ClassNameGeneration {
    fn default() -> Self {
        Self {
            template: DEFAULT_CLASS_NAME_TEMPLATE.to_owned(),
            excludes: vec![],
        }
    }
}

pub(crate) static DEFAULT_MINIFY: bool = true;

fn default_minify() -> bool {
    DEFAULT_MINIFY
}

#[derive(Deserialize, Debug, Clone)]
pub struct Settings {
    #[serde(default)]
    pub(crate) debug: bool,
    #[serde(default = "default_minify")]
    pub(crate) minify: bool,
    #[serde(default)]
    pub(crate) load_paths: Vec<PathBuf>,
    pub(crate) browser_targets: Option<BrowserVersions>,
    #[serde(default)]
    pub(crate) class_names: ClassNameGeneration,
    pub(crate) file_output: Option<FileOutput>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            debug: false,
            minify: DEFAULT_MINIFY,
            load_paths: Vec::new(),
            browser_targets: None,
            class_names: ClassNameGeneration::default(),
            file_output: None,
        }
    }
}

impl Settings {
    pub fn canonicalized_load_paths(&self) -> Result<Vec<PathBuf>, PathResolutionError> {
        self.load_paths
            .clone()
            .into_iter()
            .map(canonicalize)
            .collect()
    }
}

impl<'a> TryFrom<Settings> for grass::Options<'a> {
    type Error = PathResolutionError;

    fn try_from(val: Settings) -> Result<Self, PathResolutionError> {
        Ok(grass::Options::default()
            .style(grass::OutputStyle::Expanded)
            .load_paths(&val.canonicalized_load_paths()?))
    }
}

impl<'a> From<Settings> for lightningcss::printer::PrinterOptions<'a> {
    fn from(val: Settings) -> Self {
        lightningcss::printer::PrinterOptions {
            minify: val.minify,
            project_root: None,
            targets: val
                .browser_targets
                .map(From::<BrowserVersions>::from)
                .into(),
            analyze_dependencies: None,
            pseudo_classes: None,
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct BrowserVersions {
    pub android: Option<BrowserVersion>,
    pub chrome: Option<BrowserVersion>,
    pub edge: Option<BrowserVersion>,
    pub firefox: Option<BrowserVersion>,
    pub ie: Option<BrowserVersion>,
    pub ios_saf: Option<BrowserVersion>,
    pub opera: Option<BrowserVersion>,
    pub safari: Option<BrowserVersion>,
    pub samsung: Option<BrowserVersion>,
}

impl From<BrowserVersions> for lightningcss::targets::Browsers {
    fn from(value: BrowserVersions) -> Self {
        Self {
            android: value.android.map(From::<BrowserVersion>::from),
            chrome: value.chrome.map(From::<BrowserVersion>::from),
            edge: value.edge.map(From::<BrowserVersion>::from),
            firefox: value.firefox.map(From::<BrowserVersion>::from),
            ie: value.ie.map(From::<BrowserVersion>::from),
            ios_saf: value.ios_saf.map(From::<BrowserVersion>::from),
            opera: value.opera.map(From::<BrowserVersion>::from),
            safari: value.safari.map(From::<BrowserVersion>::from),
            samsung: value.samsung.map(From::<BrowserVersion>::from),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum BrowserVersion {
    Major(u8),
    MajorSingleValueArray((u8,)),
    MajorMinor(u8, u8),
    MajorMinorPatch(u8, u8, u8),
}

impl From<BrowserVersion> for u32 {
    fn from(value: BrowserVersion) -> Self {
        let version = match value {
            BrowserVersion::Major(major) => (major, 0, 0),
            BrowserVersion::MajorSingleValueArray((major,)) => (major, 0, 0),
            BrowserVersion::MajorMinor(major, minor) => (major, minor, 0),
            BrowserVersion::MajorMinorPatch(major, minor, path) => (major, minor, path),
        };
        (version.0 as u32 & 0xff) << 16 | (version.1 as u32 & 0xff) << 8 | (version.2 as u32 & 0xff)
    }
}

static TURF_SETTINGS: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();
static TURF_DEV_SETTINGS: std::sync::OnceLock<Settings> = std::sync::OnceLock::new();

#[derive(Debug, thiserror::Error)]
#[error("Could not obtain turf settings from the Cargo manifest")]
pub struct SettingsError(#[from] ManifestError);

impl Settings {
    pub fn get() -> Result<Self, SettingsError> {
        let dev_settings = Self::dev_profile_settings()?;
        let prod_settings = Self::prod_profile_settings()?;

        Ok(Self::choose_settings(
            dev_settings,
            prod_settings,
            cfg!(debug_assertions),
        ))
    }

    fn choose_settings(
        dev: Option<Settings>,
        prod: Option<Settings>,
        is_debug_build: bool,
    ) -> Self {
        if let (Some(cfg), true) = (dev.or(prod.clone()), is_debug_build) {
            cfg
        } else if let (Some(cfg), false) = (prod, is_debug_build) {
            cfg
        } else {
            Settings::default()
        }
    }

    fn dev_profile_settings() -> Result<Option<Self>, SettingsError> {
        if let Some(turf_dev_settings) = TURF_DEV_SETTINGS.get() {
            return Ok(Some(turf_dev_settings.clone()));
        }

        let dev_settings_maybe = crate::manifest::cargo_manifest()?
            .package
            .and_then(|package| package.metadata)
            .and_then(|metadata| metadata.turf_dev);

        if let Some(turf_dev_settings) = dev_settings_maybe.clone() {
            TURF_DEV_SETTINGS
                .set(turf_dev_settings)
                .expect("internal turf-dev settings have already been set, but should be empty");
        }

        Ok(dev_settings_maybe)
    }

    fn prod_profile_settings() -> Result<Option<Self>, SettingsError> {
        if let Some(turf_prod_settings) = TURF_SETTINGS.get() {
            return Ok(Some(turf_prod_settings.clone()));
        }

        let prod_settings_maybe = crate::manifest::cargo_manifest()?
            .package
            .and_then(|package| package.metadata)
            .and_then(|metadata| metadata.turf);

        if let Some(turf_prod_settings) = prod_settings_maybe.clone() {
            TURF_SETTINGS
                .set(turf_prod_settings)
                .expect("internal turf settings have already been set, but should be empty");
        }

        Ok(prod_settings_maybe)
    }
}

#[cfg(test)]
mod debug_tests {
    use crate::settings::ClassNameGeneration;

    use super::Settings;

    #[test]
    fn use_dev_settings_for_debug_build() {
        let mut dev_settings = Settings::default();
        let class_name_generation = ClassNameGeneration {
            template: String::from("abc"),
            ..Default::default()
        };
        dev_settings.class_names = class_name_generation;

        let mut prod_settings = Settings::default();
        let class_name_generation = ClassNameGeneration {
            template: String::from("def"),
            ..Default::default()
        };
        prod_settings.class_names = class_name_generation;

        let selected_settings =
            Settings::choose_settings(Some(dev_settings.clone()), Some(prod_settings), true);

        assert_eq!(selected_settings.class_names, dev_settings.class_names);
    }

    #[test]
    fn use_prod_settings_for_debug_build_when_no_dev_settings_where_given() {
        let mut prod_settings = Settings::default();
        let class_name_generation = ClassNameGeneration {
            template: String::from("def"),
            ..Default::default()
        };
        prod_settings.class_names = class_name_generation;

        let selected_settings = Settings::choose_settings(None, Some(prod_settings.clone()), true);

        assert_eq!(selected_settings.class_names, prod_settings.class_names);
    }

    #[test]
    fn use_prod_settings_for_release_build() {
        let mut dev_settings = Settings::default();
        let class_name_generation = ClassNameGeneration {
            template: String::from("abc"),
            ..Default::default()
        };
        dev_settings.class_names = class_name_generation;

        let mut prod_settings = Settings::default();
        let class_name_generation = ClassNameGeneration {
            template: String::from("def"),
            ..Default::default()
        };
        prod_settings.class_names = class_name_generation;

        let selected_settings =
            Settings::choose_settings(Some(dev_settings), Some(prod_settings.clone()), false);

        assert_eq!(selected_settings.class_names, prod_settings.class_names);
    }

    #[test]
    fn do_not_use_dev_settings_for_release_build() {
        let mut dev_settings = Settings::default();
        let class_name_generation = ClassNameGeneration {
            template: String::from("abc"),
            ..Default::default()
        };
        dev_settings.class_names = class_name_generation;

        let selected_settings = Settings::choose_settings(Some(dev_settings.clone()), None, false);

        assert_ne!(selected_settings.class_names, dev_settings.class_names);
    }
}
