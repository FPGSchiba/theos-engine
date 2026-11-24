# lore-poc fixture

Lore records and player questions for the local-LLM proof of concept: an NPC answers a question, grounded by retrieval over its people's beliefs.

**Not canon.** The records were taken by hand from the twelve Confluence lore pages on 2026-11-12, so the PoC has real material to retrieve from. The real records will come from the editor's import. Do not copy these files into `tower-of-theos`.

## Layout

| Path | What it is |
|---|---|
| `records/` | A content project in schema v1: `theos.toml`, `.theos/settings.toml`, `world/`, `people/`, `lore/`. The editor's content crate can index it. |
| `scenarios.toml` | 32 player questions, each with the records a good answer rests on and the records that must stay out. |
| `expected/visible.json` | For each speaker: its ancestors, its effective lens, the beliefs it holds and the beliefs it rejects. A golden file for the filter test. |

`records/` holds 125 records: 1 tower, 4 bands, 6 peoples, 6 factions, 8 speakers, 12 topic stubs, 25 truths, 53 beliefs and 10 clues. The format is the editor's data model (`docs/specs/data-model.md` in `theos-editor`).

## The speaker rule

This is what may enter a prompt for a speaker. Everything else is forbidden.

1. **Persona.** The speaker and its ancestors (faction, then people): title, role, body and the text fields of the effective lens.
2. **Held beliefs.** Every belief whose `held_by` names the speaker or one of its ancestors.
3. **Rejected beliefs.** Every belief in the effective lens's `rejects`. The speaker knows these and argues against them.

Never in a prompt: truths, clues and topics (they are developer voice), and beliefs of other holders. That covers every truth with `target: never` and every truth in the speaker's `unaware_of`; the test should check those two lists as well, as a second guard.

Notes on the rule:

- **Lens inheritance** works per field: a child inherits each lens field it does not set, and the nearest holder wins.
- **In-world text** of a belief is its title and its voice blocks (`> [!voice]`). The body is designer voice, written from the believers' side. Bodies in this fixture contain no developer vocabulary, so they are safe to use; whether they improve answers is a question for the PoC.
- **Stricter than the editor.** The editor withholds `never` truths and `unaware_of` truths from a draft. Here no truth reaches a speaker at all, because the model is small and runs unsupervised.
- **Closed world.** A speaker knows only what the beliefs of its holders say. Asked about another people's lore, a good answer says it does not know.

`expected/visible.json` is this rule applied to all eight speakers. It equals what `Index::lens` of the editor's `theos-content` crate returns for them.

## Scenario fields

| Field | Meaning |
|---|---|
| `npc` | The speaker, an `npc` record. |
| `question` | What the player says. |
| `behaviour` | `answer`, `unaware`, `rumour` or `deflect` (below). |
| `expect` | Beliefs a good answer rests on. Retrieval must return all of them within `top_k`. |
| `also_ok` | Beliefs the answer may also use. |
| `rejects` | Beliefs the speaker should argue against. Retrieval must return them too. |
| `forbid` | The records closest to the question that the speaker may not see. None may reach the prompt. |
| `forbid_words` | Extra words for this scenario, on top of the global list. |
| `hand_scored` | The word check does not fit; read the answer. |
| `note` | Why the scenario exists. |

The global `forbid_words` are node, edge, graph and frequency: canon says no inhabitant has ever heard them. Match them as whole words in any case. Plurals are not matched, as in the editor's rule.

| Behaviour | A good answer |
|---|---|
| `answer` | Speaks from the expected beliefs, in the speaker's voice. |
| `unaware` | Says it does not know, in voice, and invents nothing. |
| `rumour` | Voices the belief as a rumour. It neither confirms nor denies it. |
| `deflect` | Stays in voice and does not follow the player out of the world. |

## Scoring

Three checks need no model:

- **Retrieval:** every `expect` and `rejects` id is among the `top_k` records returned.
- **Leak:** every record in the prompt is in the speaker's entry of `expected/visible.json`.
- **Forbidden records:** no `forbid` id is in the prompt.

Two checks read the answer:

- **Words:** the answer contains no forbidden word.
- **Grounding:** the answer cites at least one `expect` id and nothing outside `expect`, `also_ok` and `rejects`. `unaware` and `deflect` answers cite nothing.

Read `rumour` answers and `hand_scored` scenarios by hand.

## Where the content comes from

Confluence space TOT, read on 2026-11-12. Each topic stub carries its page URL in `source`. Every belief and truth names its topic, except T-915 and T-916, which several pages state.

| Topic | Explaining page (version) | In-world page (version) |
|---|---|---|
| The Tower & the Weave | 171016193 (1) | 170917890 (1) |
| The Summit & the Endings | 170983426 (2) | 171212801 (2) |
| Runes | 169934850 (1) | 170590209 (1) |
| Chanting | 170786817 (1) | 170262576 (1) |
| Martial Arts | 170295318 (2) | 170655745 (3) |
| Spirit Magic | 170262596 (1) | 170655765 (1) |

The band records follow the Summit in-world page and the band table of the theos-design plugin.

## What the fixture adds

These parts are not in the pages:

- **Ids.** The pages have no truth, belief or clue ids. The fixture uses the 9xx block.
- **Speakers.** The eight `fx-*` records are roles, tagged `fixture`. They are not characters of the game.
- **Factions.** Hedge-Scribes, Academy-Scribes and the heretical minority of the Concordance are groups the pages describe; the fixture models them as factions.
- **`target`.** `confirmed` for every truth except T-913, which is `never` by the canon directive on the Seal. The pages set no targets.
- **`accuracy`** of beliefs, and the **gates, order and vectors** of clues where the pages are silent.
- **Lens `tier`.** T1 for five peoples and T2 for the Concordance.
- **`unaware_of`.** Every truth, for every people, from the line "No living character knows this".
- **B-907.** The pages say the plates read as holy script at low Understanding; the fixture gives that belief to the Hearth folk.
- **B-908.** The page says what the Concordance does, not what it believes; the title is the fixture's wording.

## Checks run

- The content repo's checker (`tools/repo_checks`): all 125 records pass the restricted YAML and kind rules.
- The editor's `theos-content` (commit `0a6f480`): no errors. It reports 24 `lore/three-clue` warnings, because the pages name few clues.
- `expected/visible.json` against `Index::lens`: equal for all eight speakers.

## Notes on the sources

- **The apex gods.** The Tower page says they are surviving Tuners or things that drifted in. The Summit page, which is later, says they are prior Conductors. T-911 follows the Summit page.
- **A forbidden word in a canon name.** The scholarly name "Sub-Clavicular Nodes" contains the plural of a word no inhabitant has heard. B-911 keeps the name; the word check does not match plurals.
- **Chakra names.** The Martial Arts in-world prose names the three great chakras Pineal, Sub-Clavicular and Lumbar, while its editorial note says sects name the points by location only. B-925 keeps the prose.
- **Myths without a holder.** The Tower was raised by giants, by angels or by a First Choir; it grew from a seed; it is the body of a sleeping god. The page gives these to "other traditions", so they are not beliefs here.
