use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Type)]
#[serde(default)]
pub struct TypesettingConfig {
    pub font_families: Vec<String>,
    pub dialogue_font: Option<String>,
    pub sfx_font: Option<String>,
    pub narration_font: Option<String>,
    pub font_size: Option<f64>,
    pub font_weight: Option<u16>,
}

impl Default for TypesettingConfig {
    fn default() -> Self {
        Self {
            font_families: vec!["CCWildWords".to_owned(), "Adobe 黑体 Std".to_owned()],
            dialogue_font: None,
            sfx_font: None,
            narration_font: None,
            font_size: None,
            font_weight: None,
        }
    }
}

impl TypesettingConfig {
    pub fn load() -> anyhow::Result<koharu_config::Config<Self>> {
        koharu_config::load("typesetting")
    }
}
