# Moderation example lists

Content for the nearest-neighbour moderator (`theos-ai` `KnnModerator`), loaded by `plugins/llm-poc/guard-kit` (`load_lists`). Design: `docs/superpowers/specs/2026-11-17-npc-guardrails-design.md` §5.5–5.6.

**Banned categories.** A match makes the NPC glitch, and the game shows its own message for the category:

| File | Category id | Holds |
|---|---|---|
| `self-harm.toml` | `self-harm` | Real-life first-person statements and requests for methods. |
| `hate.toml` | `hate` | Demeaning statements and requests about real-world groups, without slurs. |
| `real-world-harm.toml` | `real-world-harm` | Requests for real weapon, explosive, drug, poison or hacking instructions. |
| `sexual-minors.toml` | `sexual-minors` | Short request-shaped lines naming a sexual act category and a minor, with no description. |
| `world-break.toml` | `world-break` | What the NPC cannot know: AI, instructions, prompts, developers, "it's a game", real-world tech and people. It covers player lines and NPC reply lines. |

**`allowed.toml`.** Dark in-world talk that must reach the model: grief, death, curses, oaths, despair, in-world threats, poisons and weapons, and hostility between the Tower's peoples. It is the counterweight of the check, not banned content.

**`moderation.toml`.** The thresholds per source (`player`, `reply`).

**Rules:**
- **File fields.** Each list file has `id`, `examples` (used by the moderator) and `held_out` (never used as examples, only for testing). No line may appear in both, or twice.
- **Languages.** English and German, roughly half each.
- **Tone.** Lines are short and non-graphic: requests and statements, never explicit material or working instructions.
- **Scope.** The banned lists hold only these categories, never dark or depressing in-world topics.
- **Review.** People review the lists. Changes are re-tuned with `npc-poc moderation-tune`.
