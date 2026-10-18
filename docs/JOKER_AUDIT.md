# The Joker: RAW stress audit

The submitted twenty-level sequence is not a legal demonstration of ninth-level Favored Soul casting or universal buff sharing. Invalid prestige levels are retained in the diagnostic replay, not silently repaired.

## Sources read directly

Local source root: `data/packs/custom/original/pcgen-BahamutDragon-master/data/35e/wizards_of_the_coast/`.

- `supplement/complete_divine/cd_classes.lst`, `cd_abilities.lst`, `cd_feats.lst`: Favored Soul pp.6–10, Church Inquisitor pp.26–28, Sacred Exorcist, Divine Metamagic.
- `supplement/book_of_exalted_deeds/be_classes.lst`, `be_abilities.lst`: Exalted Arcanist pp.61–62. The ability prose restricts advancement to arcane classes; the LST's `ANY` progression is erroneous. Bonus exalted feats are at 3 and 5, despite the LST counter at 2.
- `campaign_settings/forgotten_realms/players_guide_to_faerun/pg_classes.lst`, `pg_abilities.lst`, `pg_feats.lst`: Spellguard pp.75–77; Magical Training. Spellguard has bonus metamagic feats at both 2 and 4; its second grant is missing from the named LST ability rows.
- `supplement/players_handbook_2/ph2_abilities.lst`: Deity's Favor p.43.
- `supplement/complete_arcane/car_feats.lst`: Precocious Apprentice and Sanctum Spell.
- `../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/rsrd_spells.lst`: Shield, False Life, True Strike and divine spell lists.

These are local PCGen transcriptions, not a claim of independent page-image verification. Conflicting machine tags were resolved using the accompanying feature prose. The existing War Weaver, Sacred Exorcist and Incantatrix sources remain attached to their native records.

## Complete level tables before combinations

The native database now includes Favored Soul 1–20, Church Inquisitor 1–10, Exalted Arcanist 1–5 and Spellguard of Silverymoon 1–5, alongside existing War Weaver 1–5, Sacred Exorcist 1–10 and Incantatrix 1–10. The focused test visits all 65 class levels before the combination probes. The ordinary `/classes` page exposes these tables.

Favored Soul uses the twenty explicit slots/known rows, selected known spells from the Cleric list, CHA for slots and casting eligibility, and WIS for spell DCs. Class features follow actual class levels, not advanced casting levels. Church Inquisitor advances divine casting at all ten levels; Exalted Arcanist advances arcane casting only at 2–5; Spellguard advances arcane casting at 1–5. Bonus feat pools use the corrected levels.

Complete progression tables do not mean every class ability has a combat resolver. Exalted spell-list/Spell Knowledge choices, sanctified spells, Church Inquisitor domain and supernatural actions, Spellguard Selective Spell, ward interactions and level-5 Spell Power remain descriptive. Magical Training, Precocious Apprentice and feat retraining are not automated by this case. These omissions do not justify granting their claimed consequences. The case is bounded to the supplied sequence, not a universal chronological prerequisite validator.

## Independent failures

| Point | RAW boundary |
|---|---|
| Human level 1 | Four feats need two additional feat sources; the post supplies no flaws. |
| Church Inquisitor at 5 | Knowledge (Religion) 4 is required before entry. It is not a Favored Soul class skill; four unmodified FS levels permit only 3.5 cross-class ranks. No override is listed. Divine Zone of Truth, church history and alignment also need evidence. |
| Exalted Arcanist at 6 | Knowledge (Arcana) 9 is required before entry. A level-5 character's ordinary class-skill cap is 8. Sanctum does not change skill ranks. |
| Arcane casting advancement | Knowing arcane spells does not turn Favored Soul into an arcane spellcasting class. Spell Knowledge provides two spells, not slots or base-class levels. Magical Training is not an actual Sorcerer/Wizard class level. |
| Divine Metamagic at 15 | The listed source of turn undead is Sacred Exorcist at 16. Later acquisition cannot satisfy an earlier prerequisite. |
| Spellguard | Requires an intrinsically defensive personal-range **arcane** spell. Deity's Favor applies to Favored Soul spells and does not change their divine type. |
| Deity's Favor | At actual FS 4, choose one affected ally. Actual FS 12 permits multiple affected allies; advancing spellcasting alone never grants this improvement. |

With the claimed FS target recorded, the replay rejects twelve arcane-only advancements (WW 4 + Spellguard 4 + Incantatrix 4). FS 4 + CI 1 + SE 1 contributes six casting levels arithmetically. This is **not** a repaired legal FS-6 build: earlier entry failures remain in the history.

Precocious Apprentice's termination when second-level spells become available, the Sanctum early-entry argument and retraining prerequisites are separate issues. No conclusion about those disputes is necessary to establish the failures above. Missing deity, region, organizational evidence, skill history, spell choices and Spellguard bonus feat choices remain unresolved.

## Implemented boundaries

- Save-DC ability is a separate class policy from slot/casting ability.
- Borrowed spell-list access can be scoped to a target class and casting power; adding Favored Soul does not lend the Cleric list to a Sorcerer on the same character.
- Spellguard conversion uses verified spell metadata. Shield and False Life qualify as arcane controls; True Strike does not. Divine Power remains ineligible when divine even though it intrinsically grants temporary HP.
- `CastRequest.spellguard_touch` is available in the ordinary casting form and quiescent-weaving form. The engine verifies actual Spellguard level and spell type before converting range. Tapestry retains its own recipient, effective-level and range checks.
- Deity's Favor is a read-only one-use calculation: affected-ally count, actual class-level threshold, 3 × spell level HP and spell-level minutes; the spell's own larger temporary-HP benefit carries its own duration. Equal values with different durations require adjudication. Previous-use pools are not added or overwritten by this probe.
- `/joker` is an English server-rendered ordinary form. It shows original errors, diagnostic advancement totals and the Deity's Favor boundary probe. It does not save resource or HP changes.

Sanctum outside the sanctum can make effective level 6 become 5 for the tapestry gate, while retaining a sixth-level slot cost. The existing Rainbow/War Weaver focused regression checks both quantities and rejects the inside-sanctum case. Neither Sanctum nor Practiced Spellcaster grants missing spell slots or class features.

## Validation

Only the new `joker_case` integration test and the existing `rainbow_native_progression_uses_one_casting_track` regression were run, plus a web build and GET/POST smoke checks. Three old DefenseRequest literals needed default values for already-added request fields before the selected regression would compile. No full gate was run.

Machine-readable inputs and replay: `tests/fixtures/forum-builds/joker.json` and `joker.audit.json`. Regenerate using `cargo run -p pcgen-engine --example joker_audit --offline`.

The generated report now includes the shared chronological acquisition audit. Missing original skill allocations and other evidence remain explicit; numerical replay does not certify full build legality. Data coverage, qualification checks and executable features are tracked separately.
