//! The guard switches selected with `--guards`.

use anyhow::bail;

/// Which guards are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Guards {
    /// The moderation pipeline (`mod`).
    pub moderation: bool,
    /// Safe tokenization of untrusted segments (`g1`).
    pub g1: bool,
    /// Instruction-ordering guard (`g4`).
    pub g4: bool,
    /// Name-line guard (`g9`).
    pub g9: bool,
}

impl Guards {
    const ALL: Guards = Guards {
        moderation: true,
        g1: true,
        g4: true,
        g9: true,
    };
    const INPUT: Guards = Guards {
        moderation: false,
        g1: true,
        g4: true,
        g9: true,
    };

    /// Parses `none`, `input` (g1, g4, g9), `all`, or a comma list of
    /// `mod`, `g1`, `g4`, `g9`.
    ///
    /// # Errors
    /// Fails on an empty string or an unknown id.
    pub fn parse(s: &str) -> anyhow::Result<Guards> {
        let s = s.trim();
        match s {
            "none" => return Ok(Guards::default()),
            "input" => return Ok(Guards::INPUT),
            "all" => return Ok(Guards::ALL),
            _ => {}
        }
        let mut g = Guards::default();
        for id in s.split(',').map(str::trim) {
            match id {
                "mod" => g.moderation = true,
                "g1" => g.g1 = true,
                "g4" => g.g4 = true,
                "g9" => g.g9 = true,
                other => bail!(
                    "unknown guard id {other:?} (expected none, input, all, or a list of mod, g1, g4, g9)"
                ),
            }
        }
        Ok(g)
    }

    /// The canonical label: `none`, `input`, `all`, or the ids joined by `+`.
    pub fn label(&self) -> String {
        if *self == Guards::default() {
            return "none".to_owned();
        }
        if *self == Guards::ALL {
            return "all".to_owned();
        }
        if *self == Guards::INPUT {
            return "input".to_owned();
        }
        [
            (self.moderation, "mod"),
            (self.g1, "g1"),
            (self.g4, "g4"),
            (self.g9, "g9"),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, id)| *id)
        .collect::<Vec<_>>()
        .join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_parse_presets_and_lists_and_reject_unknown_ids() {
        assert_eq!(Guards::parse("none").unwrap(), Guards::default());
        let input = Guards::parse("input").unwrap();
        assert!(input.g1 && input.g4 && input.g9 && !input.moderation);
        let all = Guards::parse("all").unwrap();
        assert!(all.g1 && all.g4 && all.g9 && all.moderation);
        let some = Guards::parse("mod, g4").unwrap();
        assert!(some.moderation && some.g4 && !some.g1 && !some.g9);
        assert!(Guards::parse("g2").is_err());
        assert!(Guards::parse("").is_err());
        assert!(Guards::parse("mod,,g1").is_err());
    }

    #[test]
    fn guards_label_is_canonical() {
        assert_eq!(Guards::default().label(), "none");
        assert_eq!(Guards::parse("g9,g1,g4").unwrap().label(), "input");
        assert_eq!(Guards::parse("g4,mod,g9,g1").unwrap().label(), "all");
        assert_eq!(Guards::parse("g4,mod").unwrap().label(), "mod+g4");
        assert_eq!(Guards::parse("g9").unwrap().label(), "g9");
    }
}
