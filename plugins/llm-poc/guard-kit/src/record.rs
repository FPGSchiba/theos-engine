//! A moderator wrapper that records the evidence of every check.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use theos_ai::{
    ContentModerator, KnnEvidence, KnnModerator, ModerationError, ModerationSource,
    ModerationVerdict,
};

/// One recorded check.
#[derive(Clone, Debug)]
pub struct CheckRecord {
    /// `<scenario>/<scope>` at the time of the check.
    pub tag: String,
    /// Where the text came from.
    pub source: ModerationSource,
    /// The full text that was checked.
    pub text: String,
    /// What the moderator found.
    pub evidence: KnnEvidence,
    /// How long the check took.
    pub elapsed: Duration,
}

/// The current tag, shared between the run loop and the worker thread.
#[derive(Clone, Debug, Default)]
pub struct Tag(Arc<Mutex<String>>);

impl Tag {
    /// Sets the tag.
    pub fn set(&self, tag: impl Into<String>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = tag.into();
    }

    /// The current tag.
    pub fn get(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// All recorded checks, shared between the worker thread and the report writer.
#[derive(Clone, Debug, Default)]
pub struct Records(Arc<Mutex<Vec<CheckRecord>>>);

impl Records {
    /// Appends a record.
    pub fn push(&self, record: CheckRecord) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(record);
    }

    /// A copy of everything recorded so far.
    pub fn snapshot(&self) -> Vec<CheckRecord> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// How many checks were recorded.
    pub fn len(&self) -> usize {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A [`KnnModerator`] that records every check it makes.
pub struct RecordingModerator {
    inner: KnnModerator,
    tag: Tag,
    records: Records,
}

impl RecordingModerator {
    /// Wraps `inner`; checks are recorded under `tag` into `records`.
    pub fn new(inner: KnnModerator, tag: Tag, records: Records) -> Self {
        Self {
            inner,
            tag,
            records,
        }
    }
}

impl ContentModerator for RecordingModerator {
    fn check(
        &mut self,
        source: ModerationSource,
        text: &str,
    ) -> Result<ModerationVerdict, ModerationError> {
        let start = Instant::now();
        let evidence = self.inner.evaluate(source, text)?;
        let elapsed = start.elapsed();
        let verdict = evidence.verdict.clone();
        self.records.push(CheckRecord {
            tag: self.tag.get(),
            source,
            text: text.to_owned(),
            evidence,
            elapsed,
        });
        Ok(verdict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use theos_ai::testkit::FakeEmbedder;
    use theos_ai::{KnnThresholds, ModerationCategory, SourceThreshold};

    fn moderator(tag: &Tag, records: &Records) -> RecordingModerator {
        let t = SourceThreshold {
            min_similarity: 0.5,
            margin: 0.0,
        };
        let knn = KnnModerator::new(
            Box::new(FakeEmbedder::default()),
            vec![(
                ModerationCategory::Hate,
                vec!["hate the foreigners".to_owned()],
            )],
            vec!["good morning friend".to_owned()],
            KnnThresholds {
                player: t,
                reply: t,
            },
        )
        .unwrap();
        RecordingModerator::new(knn, tag.clone(), records.clone())
    }

    #[test]
    fn recording_moderator_records_every_check_with_its_tag() {
        let tag = Tag::default();
        let records = Records::default();
        let mut m = moderator(&tag, &records);
        tag.set("s1/player");
        let v = m
            .check(ModerationSource::PlayerMessage, "hate the foreigners")
            .unwrap();
        assert_eq!(v, ModerationVerdict::Block(ModerationCategory::Hate));
        tag.set("s2/reply");
        let v = m
            .check(ModerationSource::ModelReply, "good morning friend")
            .unwrap();
        assert_eq!(v, ModerationVerdict::Allow);

        let r = records.snapshot();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].tag, "s1/player");
        assert_eq!(r[0].source, ModerationSource::PlayerMessage);
        assert_eq!(r[0].text, "hate the foreigners");
        assert_eq!(
            r[0].evidence.verdict,
            ModerationVerdict::Block(ModerationCategory::Hate)
        );
        assert_eq!(r[1].tag, "s2/reply");
        assert_eq!(r[1].evidence.verdict, ModerationVerdict::Allow);
        assert!(!records.is_empty());
    }
}
