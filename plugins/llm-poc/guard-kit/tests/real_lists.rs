//! The real moderation example lists in `fixtures/moderation` load under the
//! loader's rules and meet the size rules of the plan (Task 9).

use std::path::Path;

use guard_kit::load_lists;
use theos_ai::ModerationCategory;

#[test]
fn real_lists_load_and_meet_the_size_rules() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/moderation");
    let lists = load_lists(&dir).expect("fixtures/moderation loads");

    let ids: Vec<&str> = lists.banned.iter().map(|c| c.category.id()).collect();
    for id in [
        "self-harm",
        "hate",
        "real-world-harm",
        "sexual-minors",
        "world-break",
    ] {
        assert!(ids.contains(&id), "category `{id}` missing: {ids:?}");
    }
    assert_eq!(lists.banned.len(), 5);
    for c in &lists.banned {
        assert!(
            !matches!(c.category, ModerationCategory::Custom(_)),
            "{} is not a built-in category",
            c.category.id()
        );
        assert!(
            c.examples.len() >= 20,
            "{}: {} examples",
            c.category.id(),
            c.examples.len()
        );
        assert!(
            c.held_out.len() >= 6,
            "{}: {} held out",
            c.category.id(),
            c.held_out.len()
        );
    }
    assert!(
        lists.allowed.len() >= 30,
        "allowed: {}",
        lists.allowed.len()
    );
    assert!(
        lists.allowed_held_out.len() >= 12,
        "allowed held out: {}",
        lists.allowed_held_out.len()
    );

    for t in [lists.thresholds.player, lists.thresholds.reply] {
        assert!(t.min_similarity > 0.0 && t.min_similarity <= 1.0);
        assert!(t.margin >= 0.0 && t.margin < 1.0);
    }
}
