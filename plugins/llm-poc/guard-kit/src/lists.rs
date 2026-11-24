//! Loading the moderation example lists from `fixtures/moderation`.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{bail, Context};
use serde::Deserialize;
use theos_ai::{KnnThresholds, ModerationCategory, SourceThreshold};

/// The five banned category files, in report order.
const CATEGORY_IDS: [&str; 5] = [
    "self-harm",
    "hate",
    "real-world-harm",
    "sexual-minors",
    "world-break",
];

/// The examples of one banned category.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryList {
    /// The category.
    pub category: ModerationCategory,
    /// Lines the moderator compares against.
    pub examples: Vec<String>,
    /// Lines kept back to measure the moderator.
    pub held_out: Vec<String>,
}

/// Everything loaded from the moderation directory.
#[derive(Clone, Debug, PartialEq)]
pub struct ModerationLists {
    /// One list per banned category.
    pub banned: Vec<CategoryList>,
    /// Lines that must pass.
    pub allowed: Vec<String>,
    /// Allowed lines kept back to measure the moderator.
    pub allowed_held_out: Vec<String>,
    /// The thresholds from `moderation.toml`.
    pub thresholds: KnnThresholds,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListFile {
    id: String,
    examples: Vec<String>,
    #[serde(default)]
    held_out: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdFile {
    player: ThresholdEntry,
    reply: ThresholdEntry,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThresholdEntry {
    min_similarity: f32,
    margin: f32,
}

impl From<ThresholdEntry> for SourceThreshold {
    fn from(e: ThresholdEntry) -> Self {
        SourceThreshold {
            min_similarity: e.min_similarity,
            margin: e.margin,
        }
    }
}

fn read_list(dir: &Path, name: &str) -> anyhow::Result<ListFile> {
    let path = dir.join(format!("{name}.toml"));
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let file: ListFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    if file.id != name {
        bail!(
            "{}: id {:?} does not match the file name",
            path.display(),
            file.id
        );
    }
    if file.examples.is_empty() {
        bail!("{}: `examples` is empty", path.display());
    }
    let mut seen = HashSet::new();
    for line in file.examples.iter().chain(file.held_out.iter()) {
        if !seen.insert(line.as_str()) {
            bail!(
                "{}: line appears twice (duplicate or in both examples and held_out): {line:?}",
                path.display()
            );
        }
    }
    Ok(file)
}

/// Loads the lists and thresholds from `dir` (`fixtures/moderation`).
///
/// All five category files, `allowed.toml` and `moderation.toml` must exist.
///
/// # Errors
/// Fails on a missing or unknown `.toml` file, an empty `examples`, a line in
/// both `examples` and `held_out`, or a duplicate line within a file.
pub fn load_lists(dir: &Path) -> anyhow::Result<ModerationLists> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if !CATEGORY_IDS.contains(&stem) && stem != "allowed" && stem != "moderation" {
            bail!("unknown file in {}: {}", dir.display(), path.display());
        }
    }
    let mut banned = Vec::new();
    for id in CATEGORY_IDS {
        let f = read_list(dir, id)?;
        banned.push(CategoryList {
            category: ModerationCategory::from_id(id),
            examples: f.examples,
            held_out: f.held_out,
        });
    }
    let allowed = read_list(dir, "allowed")?;
    let path = dir.join("moderation.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let t: ThresholdFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(ModerationLists {
        banned,
        allowed: allowed.examples,
        allowed_held_out: allowed.held_out,
        thresholds: KnnThresholds {
            player: t.player.into(),
            reply: t.reply.into(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!(
            "guard-kit-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    fn fixture() -> PathBuf {
        let d = temp_dir();
        for id in CATEGORY_IDS {
            write(
                &d,
                &format!("{id}.toml"),
                &format!(
                    "id = \"{id}\"\nexamples = [\"{id} one\", \"{id} two\"]\nheld_out = [\"{id} held\"]\n"
                ),
            );
        }
        write(
            &d,
            "allowed.toml",
            "id = \"allowed\"\nexamples = [\"hello there\"]\nheld_out = [\"good day\"]\n",
        );
        write(
            &d,
            "moderation.toml",
            "[player]\nmin_similarity = 0.7\nmargin = 0.05\n[reply]\nmin_similarity = 0.8\nmargin = 0.1\n",
        );
        write(&d, "README.md", "ignored");
        d
    }

    #[test]
    fn load_lists_reads_a_temp_fixture() {
        let d = fixture();
        let l = load_lists(&d).unwrap();
        assert_eq!(l.banned.len(), 5);
        assert_eq!(l.banned[0].category, ModerationCategory::SelfHarm);
        assert_eq!(l.banned[0].examples, ["self-harm one", "self-harm two"]);
        assert_eq!(l.banned[0].held_out, ["self-harm held"]);
        assert_eq!(l.allowed, ["hello there"]);
        assert_eq!(l.allowed_held_out, ["good day"]);
        assert_eq!(l.thresholds.player.min_similarity, 0.7);
        assert_eq!(l.thresholds.reply.margin, 0.1);
    }

    #[test]
    fn load_lists_rejects_overlap_and_duplicates_and_empty() {
        let d = fixture();
        write(&d, "stray.toml", "id = \"stray\"\nexamples = [\"x\"]\n");
        assert!(load_lists(&d)
            .unwrap_err()
            .to_string()
            .contains("unknown file"));
        std::fs::remove_file(d.join("stray.toml")).unwrap();

        write(&d, "hate.toml", "id = \"hate\"\nexamples = []\n");
        assert!(load_lists(&d).unwrap_err().to_string().contains("empty"));

        write(
            &d,
            "hate.toml",
            "id = \"hate\"\nexamples = [\"a\"]\nheld_out = [\"a\"]\n",
        );
        assert!(load_lists(&d).is_err());

        write(
            &d,
            "hate.toml",
            "id = \"hate\"\nexamples = [\"a\", \"a\"]\n",
        );
        assert!(load_lists(&d).is_err());

        write(&d, "hate.toml", "id = \"hate\"\nexamples = [\"a\"]\n");
        assert!(load_lists(&d).is_ok());

        std::fs::remove_file(d.join("world-break.toml")).unwrap();
        assert!(load_lists(&d).is_err());
    }
}
