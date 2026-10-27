use revision::revisioned;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    Result,
    component::{Component, ValidationContext},
    components::{GlossaryEntryId, Origin, validate_multiline_text, validate_text},
};

#[revisioned(revision = 1)]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CharacterGender {
    #[default]
    Unspecified,
    Male,
    Female,
    NonBinary,
}

#[revisioned(revision = 2)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CharacterProfile {
    pub id: GlossaryEntryId,
    pub gender: CharacterGender,
    pub first_person: Option<String>,
    pub second_person: Option<String>,
    pub tone: Option<String>,
    pub custom_instructions: Option<String>,
    #[revision(start = 2)]
    #[serde(default)]
    pub honorific_policy: Option<String>,
}

impl CharacterProfile {
    pub fn validate(&self) -> Result<()> {
        if let Some(first_person) = &self.first_person {
            validate_text(first_person, 128, false, "first_person")?;
        }
        if let Some(second_person) = &self.second_person {
            validate_text(second_person, 128, false, "second_person")?;
        }
        if let Some(tone) = &self.tone {
            validate_text(tone, 256, false, "tone")?;
        }
        if let Some(instructions) = &self.custom_instructions {
            validate_multiline_text(instructions, 2048, false, "custom_instructions")?;
        }
        if let Some(policy) = &self.honorific_policy {
            validate_text(policy, 128, false, "character honorific_policy")?;
        }
        Ok(())
    }
}

#[revisioned(revision = 1)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Speaker {
    pub origin: Origin,
    pub speaker: Option<GlossaryEntryId>,
}

impl Default for Speaker {
    fn default() -> Self {
        Self {
            origin: Origin::User,
            speaker: None,
        }
    }
}

impl Speaker {
    #[must_use]
    pub fn new(speaker: Option<GlossaryEntryId>, origin: Origin) -> Self {
        Self { origin, speaker }
    }

    #[must_use]
    pub const fn id(&self) -> Option<GlossaryEntryId> {
        self.speaker
    }
}

impl Component for Speaker {
    const KIND: &'static str = "dev.koharu.text.speaker";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        self.origin.validate()
    }

    fn origin(&self) -> Option<&Origin> {
        Some(&self.origin)
    }

    fn set_origin(&mut self, origin: Origin) -> bool {
        self.origin = origin;
        true
    }
}
