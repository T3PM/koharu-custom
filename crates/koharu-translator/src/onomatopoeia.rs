use crate::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnomatopoeiaEntry {
    pub source: &'static str,
    pub en: &'static str,
    pub description: &'static str,
}

pub static ONOMATOPOEIA_TABLE: &[OnomatopoeiaEntry] = &[
    OnomatopoeiaEntry {
        source: "ドキドキ",
        en: "thump thump",
        description: "heart beating fast / nervous",
    },
    OnomatopoeiaEntry {
        source: "どきどき",
        en: "thump thump",
        description: "heart beating fast / nervous",
    },
    OnomatopoeiaEntry {
        source: "ドキッ",
        en: "thump",
        description: "heart skip / startled",
    },
    OnomatopoeiaEntry {
        source: "どきっ",
        en: "thump",
        description: "heart skip / startled",
    },
    OnomatopoeiaEntry {
        source: "ドン",
        en: "boom",
        description: "heavy impact / thud",
    },
    OnomatopoeiaEntry {
        source: "どん",
        en: "boom",
        description: "heavy impact / thud",
    },
    OnomatopoeiaEntry {
        source: "バン",
        en: "bang",
        description: "loud sound / slam",
    },
    OnomatopoeiaEntry {
        source: "ばーん",
        en: "bam",
        description: "dramatic entrance / reveal",
    },
    OnomatopoeiaEntry {
        source: "バーン",
        en: "bam",
        description: "dramatic entrance / reveal",
    },
    OnomatopoeiaEntry {
        source: "ざわ",
        en: "murmur",
        description: "crowd muttering / uneasy atmosphere",
    },
    OnomatopoeiaEntry {
        source: "ざわざわ",
        en: "murmur",
        description: "unsettled crowd / commotion",
    },
    OnomatopoeiaEntry {
        source: "ゴゴゴ",
        en: "rumble",
        description: "menacing aura / vibration",
    },
    OnomatopoeiaEntry {
        source: "ごごご",
        en: "rumble",
        description: "menacing aura / vibration",
    },
    OnomatopoeiaEntry {
        source: "ニコ",
        en: "smile",
        description: "friendly smile",
    },
    OnomatopoeiaEntry {
        source: "にこにこ",
        en: "grin",
        description: "beaming smile",
    },
    OnomatopoeiaEntry {
        source: "ギラ",
        en: "glint",
        description: "sharp glare / shining",
    },
    OnomatopoeiaEntry {
        source: "パチパチ",
        en: "clap clap",
        description: "clapping / crackling",
    },
    OnomatopoeiaEntry {
        source: "チュッ",
        en: "smooch",
        description: "kiss",
    },
    OnomatopoeiaEntry {
        source: "ハッ",
        en: "gasp",
        description: "sudden realization / surprise",
    },
    OnomatopoeiaEntry {
        source: "クスッ",
        en: "chuckle",
        description: "stifled laugh / giggle",
    },
    OnomatopoeiaEntry {
        source: "シーン",
        en: "silence",
        description: "awkward silence",
    },
    OnomatopoeiaEntry {
        source: "ビクッ",
        en: "twitch",
        description: "startled jump / flinch",
    },
    OnomatopoeiaEntry {
        source: "ギュッ",
        en: "squeeze",
        description: "tight hug / squeeze",
    },
    OnomatopoeiaEntry {
        source: "タタタ",
        en: "patter",
        description: "running footsteps",
    },
    OnomatopoeiaEntry {
        source: "ガーン",
        en: "shock",
        description: "devastation / shocked dismay",
    },
    OnomatopoeiaEntry {
        source: "ガタッ",
        en: "clatter",
        description: "sudden shake / rattle",
    },
    OnomatopoeiaEntry {
        source: "ポロポロ",
        en: "drip drip",
        description: "tears falling",
    },
];

fn normalize_onomatopoeia_key(s: &str) -> String {
    let stripped = s
        .trim()
        .trim_end_matches(['!', '?', '！', '？', '〜', 'ー', '…', '.', '。', '、'])
        .trim();
    stripped
        .chars()
        .map(|c| {
            if ('\u{3041}'..='\u{3096}').contains(&c) {
                char::from_u32(c as u32 + 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

#[must_use]
pub fn lookup_onomatopoeia(source: &str, target: Language) -> Option<&'static str> {
    let trimmed = source
        .trim()
        .trim_end_matches(['!', '?', '！', '？', '〜', 'ー', '…', '.', '。', '、'])
        .trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = normalize_onomatopoeia_key(trimmed);
    let entry = ONOMATOPOEIA_TABLE.iter().find(|e| {
        e.source == trimmed || normalize_onomatopoeia_key(e.source) == normalized
    })?;
    let _ = entry.description;
    match target {
        Language::English => Some(entry.en),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_onomatopoeia() {
        assert_eq!(
            lookup_onomatopoeia(" ドキドキ ", Language::English),
            Some("thump thump")
        );
        assert_eq!(
            lookup_onomatopoeia("どきどき！", Language::English),
            Some("thump thump")
        );
        assert_eq!(
            lookup_onomatopoeia("ゴゴゴ", Language::English),
            Some("rumble")
        );
        assert_eq!(
            lookup_onomatopoeia("ぱちぱち…", Language::English),
            Some("clap clap")
        );
        assert_eq!(lookup_onomatopoeia("hello", Language::English), None);
    }
}
