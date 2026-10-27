use koharu_scene::{BalloonShape, FontStyle};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Type)]
#[serde(default)]
pub struct TypesettingConfig {
    pub font_families: Vec<String>,
    pub dialogue_font: Option<String>,
    pub sfx_font: Option<String>,
    pub narration_font: Option<String>,
    pub shout_font: Option<String>,
    pub thought_font: Option<String>,
    pub whisper_font: Option<String>,
    pub font_size: Option<f64>,
    pub font_weight: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct TypesettingPreset {
    pub font_family: Option<String>,
    pub font_weight: Option<u16>,
    pub font_style: Option<FontStyle>,
    pub size_scale: f32,
    pub stroke_width: Option<f32>,
}

impl Default for TypesettingConfig {
    fn default() -> Self {
        Self {
            font_families: vec!["CCWildWords".to_owned(), "Adobe 黑体 Std".to_owned()],
            dialogue_font: None,
            sfx_font: None,
            narration_font: None,
            shout_font: None,
            thought_font: None,
            whisper_font: None,
            font_size: None,
            font_weight: None,
        }
    }
}

impl TypesettingConfig {
    pub fn load() -> anyhow::Result<koharu_config::Config<Self>> {
        koharu_config::load("typesetting")
    }

    #[must_use]
    pub fn preset_for_shape(&self, shape: BalloonShape) -> TypesettingPreset {
        match shape {
            BalloonShape::NormalSpeech => TypesettingPreset {
                font_family: self
                    .dialogue_font
                    .clone()
                    .or_else(|| self.font_families.first().cloned()),
                font_weight: self.font_weight.or(Some(400)),
                font_style: None,
                size_scale: 1.0,
                stroke_width: None,
            },
            BalloonShape::Shout => TypesettingPreset {
                font_family: self
                    .shout_font
                    .clone()
                    .or_else(|| self.dialogue_font.clone())
                    .or_else(|| self.font_families.first().cloned()),
                font_weight: self.font_weight.or(Some(700)),
                font_style: None,
                size_scale: 1.15,
                stroke_width: Some(2.0),
            },
            BalloonShape::Thought => TypesettingPreset {
                font_family: self
                    .thought_font
                    .clone()
                    .or_else(|| self.dialogue_font.clone())
                    .or_else(|| self.font_families.first().cloned()),
                font_weight: self.font_weight.or(Some(400)),
                font_style: Some(FontStyle::Italic),
                size_scale: 0.95,
                stroke_width: None,
            },
            BalloonShape::Whisper => TypesettingPreset {
                font_family: self
                    .whisper_font
                    .clone()
                    .or_else(|| self.dialogue_font.clone())
                    .or_else(|| self.font_families.first().cloned()),
                font_weight: self.font_weight.or(Some(300)),
                font_style: None,
                size_scale: 0.85,
                stroke_width: None,
            },
            BalloonShape::Narration => TypesettingPreset {
                font_family: self
                    .narration_font
                    .clone()
                    .or_else(|| self.font_families.first().cloned()),
                font_weight: self.font_weight.or(Some(400)),
                font_style: None,
                size_scale: 1.0,
                stroke_width: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use koharu_scene::BalloonShape;

    use super::TypesettingConfig;

    #[test]
    fn configured_weight_overrides_shape_defaults() {
        let config = TypesettingConfig {
            font_weight: Some(600),
            ..TypesettingConfig::default()
        };

        assert_eq!(
            config.preset_for_shape(BalloonShape::Shout).font_weight,
            Some(600)
        );
        assert_eq!(
            config.preset_for_shape(BalloonShape::Whisper).font_weight,
            Some(600)
        );
    }
}
