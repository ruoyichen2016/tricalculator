# Third-party packs, transcribed into the canonical format

`data/packs/custom/` holds other publishers' PCGen data: 393 `.pcc` files across
five collections, 161 MB of `.lst` source. This note records the run that
transcribed them into the normalised pack format, what it produced, and what is
still missing.

The transcriptions are **local-only**. The sources are copyrighted commercial
material, so the output lives in `data/packs/transcribed/`, which `.gitignore`
excludes. Nothing here is published.

## Running it

```sh
# one pack
cargo run -q -p pcgen-cli -- transpile --pcc <campaign>.pcc --out data/packs/transcribed/<name>

# read it back without re-interpreting any PCGen text
cargo run -q -p pcgen-cli -- pack-info --pack data/packs/transcribed/<name> --json
```

Transcription is deliberately a separate, expensive step: `transpile` interprets
the PCGen text once and persists the result; `pack-info` then loads that
directory without parsing anything. That split is what makes the round-trip check
below meaningful — a pack that writes but does not read back is a broken build
artefact, not a data problem.

## Scope: 196 campaigns, not 393 files

Most `.pcc` files are sub-packs included by a parent, so transcribing all 393
would duplicate their contents many times over. The run transcribes only the
**top-level** campaigns — those no other `.pcc` includes — which is 196 of the
393. The include graph is resolved by reading each file's `PCC:` lines.

## Result

| | |
|---|---|
| Top-level campaigns | 196 |
| Transcribed | **189** |
| Failed | 7 |
| Entities | 342,934 |
| Tag occurrences | 1,762,682 |
| Unsupported tags | 1,409 (**0.08%**) |
| Legacy (recognised, carried) tags | 115,333 |
| Round-trip (`pack-info` reads it back) | **189 / 189** |
| Output size | 635 MB |

## What blocked it, and what that changed

The first run transcribed 169 of 196. Three reading defects accounted for the
rest, and all three were fixed rather than worked around:

- **`PREABILITY`, `PRESKILL`, `PRECLASS` value forms.** PCGen splits a
  prerequisite part on *every* `=`, reads the value from the **last** segment,
  and rebuilds the key from the rest. Splitting at the first `=` silently dropped
  `Weapon Focus (TYPE=Bow)`, `TYPE=Perform=8` and `Knowledge (TYPE=Other)=3`, and
  a bare `PRECLASS:1,Bard` was rejected outright. See
  `PCGEN_SYNTAX_INVENTORY.md` §5t.
- **`PRETEXT` inside a `BONUS`.** It is a documented no-op — "does not disqualify
  anything" — but as an unknown predicate it discarded the whole effect it gated.
  7,358 occurrences. Across all 393 packs this alone took unsupported tags from
  9,409 to 2,035.
- **Windows-1252 data.** 20 of the 27 failures were `.lst` files that are not
  valid UTF-8. `pcgen_import::text::read_text` now tries UTF-8 and falls back to
  Windows-1252, which recovered *Book of Exalted Deeds*, *Frostburn*, *Races of
  the Wild* and 17 others.

One further defect only the round-trip check could find: 16 packs **wrote but
would not load back**, because a variant's owning class had been inferred from a
pool or group name (`KEY:ACF ~ Bard Inspire Competence` → class `ACF`). The
loader refuses a variant naming an undeclared class. The owner is now derived
from the displaced-feature anchor (`BardInspireCompetence` → `Bard`) and, when it
cannot be substantiated by a class the pack declares, dropped instead of written.

## What is still unsupported

> **The table below is the original transcription snapshot**, taken while several
> of these tags were still unimplemented. It is kept as the record of what the
> run found, not as the current state; the queue is re-measured with
> `pcgen import-report <pcc> --histogram` over the 189 campaigns, and the
> reductions since are listed under it.

1,409 tags (0.08% of occurrences) across the 189 packs:

| Tag | Count | Note |
|---|---|---|
| `BONUS` | 599 | gate unreadable; effect is retained and the raw gate is flagged |
| `PREMULT` | 387 | grouping operator |
| `PRELEVELMAX` | 190 | |
| `PREREGION` | 65 | |
| `PREATT` | 27 | |
| `!PRECSKILL` | 18 | |
| `PRECHECK` | 17 | |
| `PRESPELLDESCRIPTOR` | 14 | |
| `PRECSKILL` | 12 | |
| `PREMOVE` | 12 | |
| `PRECAMPAIGN`, `PREHANDSGT`, `PRESPELLSCHOOLSUB`, `PRECHARACTERTYPE` | 9 each | |
| `PREALIGN` | 8 | |
| `PRESA` | 7 | |
| `PREUATT` | 4 | |
| `PREDR`, `PREREACHGTEQ`, `PRESKILLTOT` | 3 each | |
| `PRECLASSLEVELMAX`, `PREITEM` | 2 each | |

`BONUS` was the one worth attention first, and it is now handled: an unreadable
gate no longer makes `lower_bonus_effect` discard the entire bonus. The effect is
retained with a `Predicate::Unsupported { raw }` condition, the engine treats that
condition as satisfied, and the raw gate still appears in the unsupported report.
This is an intentional visible over-approximation for third-party packs; it is a
smaller lie than silently dropping a bonus.

The 1,409 unsupported count above therefore still measures uninterpreted tags,
but no longer measures silently lost `BONUS` / `TEMPBONUS` effects.

After that run, several listed predicates were lowered, and `PRELEVEL` now
honours both `MIN=` and `MAX=`.  The table above is the original transcription
snapshot and has not been re-counted; the underlying source run would need to be
repeated to freeze a new unsupported total.

Subsequent reductions not reflected above:

- `PRELEVELMAX` (190 tags);
- `PREATT` (27 tags), aliased to the existing `PRETOTALAB` predicate;
- `PREHANDSGT` (9 tags), lowered to `HandsAtLeast { n + 1 }`;
- multi-mode `PREMOVE` (12 tags), now represented as `MovementCount`;
- `PREREGION` (65 tags), lowered to `Predicate::Region` against a new
  template `REGION` / `SUBREGION` model. It also took `PREMULT` from 387 to 31:
  a `PREMULT` whose *inner* predicate was unreadable was itself unsupported, so
  356 grouping operators cleared with it;
- `PRECSKILL` / `!PRECSKILL` (30 tags), lowered to `Predicate::ClassSkill`
  against a class-skill set built from the character's classes. It also took
  `BONUS` from 599 to 280, because a `BONUS` whose gate is unreadable is
  reported under its own tag name: 319 of them were gated on `PRECSKILL`.
  Not modelled: the global `CSKILL:` / `CCSKILL:` that PCGen also accepts on
  templates, races and abilities, and `SERVESAS` imitators.

The `BONUS` drop is a reporting change, not a lost effect, and it was checked
rather than assumed: on `homebrew_requests`, whose 99 `BONUS` rows all carry a
`PRECSKILL` gate, `lowered_effects` is 117 before and after while
`unsupported_tags` goes 99 to 0;
- `PREALIGN` (8 tags), the numeric keys (`PREALIGN:0,1,3,4,6,7` in the Book of
  Erotic Fantasy). They were being *dropped*, which left those prestige classes
  with no alignment restriction; they are now kept as keys no alignment file
  defines, so the requirement fails closed. `PREALIGN:Deity` — the one special
  case in `PreAlignTester` — is also implemented now;
- `PRECHECK` (17 tags), lowered to `Predicate::Check` against a new
  `EffectState.total_saves` (base save plus the save's ability modifier). It also
  took `PREMULT` from 31 to 28, for the usual reason: a `PREMULT` whose inner
  predicate cannot be read is itself unsupported. The same pass fixed
  `PRECHECKBASE`, which had been dropping the reference's count — see §5x;
- `PRESPELLDESCRIPTOR` (14 tags), lowered to `Predicate::SpellDescriptor` against
  a new `EffectState.spell_descriptors`. `PREMULT` went 28 to 19 with it;
- `PRESPELLSCHOOLSUB` (9 tags), lowered to `Predicate::SpellSchoolSub` against
  `EffectState.spell_subschools` — the same counting tester with the sub-school
  slot of `aggregateSpellList` filled in;
- not a reduction but a correction, `PREVARGT`/`PREVARGTEQ` and `PREVARLT`/`PREVARLTEQ`:
  the four kinds differ only in their boundary, and the symbolic form of all four
  lowered to one shape that the engine read as strict. **2,519** pairs in these
  packs have a symbolic operand and were compared one step too strictly. Nothing
  moves in the table below, because `PREVAR*` already lowered;
- `PRECLASSLEVELMAX` (2 tags), lowered to the negated `ClassRequirement` the reference
  rewrites it into — this tag has no tester of its own;
- `PREITEM` (2 tags), lowered to `Predicate::Item`. The old lowering asked whether an
  item was *equipped* and understood only `TYPE=` entries, so the name entries were
  unsupported and the type entry asked the wrong question;
- `PREREACHGTEQ` (3 tags), lowered to `Predicate::ReachAtLeast` — and, because the
  reference reads the comparison out of the tag name, the other six kinds of the
  family with it. Pending the batch verification, so the total below does not yet
  include it;
- `PREDR` (3 tags), lowered to `Predicate::DamageReduction`. Pending the batch
  verification described at the top of `CHANGELOG.md`, so the total below does not
  yet include it;
- `PREFACT` (never counted on its own, but the gate behind **245** `BONUS` tags and
  **18** `PREMULT` tags), lowered to `Predicate::Fact` against a new `FACT` model.
  `BONUS` fell from 268 to 23 and `PREMULT` from 19 to 2 — the single largest
  reduction in this loop, and the reason the numbers below move so far;
- `PRESA` (7 tags), lowered to `Predicate::SpecialAbility`. It needed a model that
  did not exist: `SA:`/`SAB:` were not recognised at all — not even as unsupported,
  because they are in none of the reported families — so the **2,378** of them in
  these packs were silently dropped. They now become granted special abilities;
- `PRECHARACTERTYPE` (9 tags), lowered to `Predicate::CharacterType` against a new
  `CharacterDraft.character_type`, which the `.pcg` reader now fills from
  `CHARACTERTYPE:`. It also took `BONUS` from 280 to 268;
- `PRECAMPAIGN` (9 tags), lowered to `Predicate::Campaign` against a new
  `CompiledPackHeader.campaigns` list built from the `.pcc` graph. Worth knowing
  before reading the number as a behaviour change: all nine sit on **class-level**
  rows, and the engine does not yet evaluate a `CLASS_LEVEL` entity's
  requirements, so the gate is now preserved rather than acted on. See
  `PCGEN_SYNTAX_INVENTORY.md` §5aa;
- and, not a reduction but a correction, `PRESPELLSCHOOL`: it was never in the
  unsupported queue because it lowered, but it matched the school name against the
  spell's *name* and dropped the reference's count. All **504** occurrences in
  these packs now evaluate against the spell's `SCHOOL:`. The total below is
  unchanged by it.

**Current total, re-measured over the same 189 campaigns: 42 unsupported
occurrences across 8 tag kinds**, against 304 / 8 after the `PRESA` round,
311 / 9 after the `PRECHARACTERTYPE` round, 332 / 10 after the `PRECAMPAIGN`
round, 341 / 11 after the `PRESPELLSCHOOL` round, 350 / 12 after the
`PRESPELLDESCRIPTOR` round, 373 / 13 after the `PRECHECK` round, 393 / 14 after
the `PREALIGN` round, 401 / 15 after the `PRECSKILL` round, 750 / 17 after the
`PREREGION` round, 1,171 / 18 after the `PREATT` round and 1,409 / 20 at
transcription. `BONUS` is 23 of the 42 and is the visible over-approximation
described above — the effect is kept and the unreadable gate is what is reported —
so the remaining actionable queue is the 19 occurrences behind it: `PREUATT` (4),
`PREDR` (3), `PREREACHGTEQ` (3), `PRESKILLTOT` (3), `PRECLASSLEVELMAX` (2),
`PREITEM` (2), `PREMULT` (2).

## The 7 campaigns that could not be transcribed

Every one is a **missing source file** in the checkout, not a parser failure:

| Campaign | Missing |
|---|---|
| `2e/for_gold_and_glory/core` | `core_abilitycategories.lst` |
| `35e/paizo/dragon_magazine` | `dm336_abilities.lst` |
| `5e/marching_modron_press/epic_characters` | `ec_abilities_hoto.lst` |
| `5e/wizards_of_the_coast/basic_rules` | `./core/universal_rules/_universal_rules.pcc` |
| `modern/green_ronin/the_psychics_handbook` | `tph_feats_hidden.lst` |
| `talislanta/morrigan_press_inc/core` | `rsrd_equip_weap_natural.lst` |
| `zen_test/blanks/xxx` | `xxx_abilities_feat.lst` |

These are incomplete third-party checkouts. Nothing in the importer can recover
them; they need the missing files.

## What this does not claim

- **The acceptance cohort is unchanged.** The coverage gate still runs over
  three sub-packs (`ds-3e`, `ds-35e`, `ds-psi`) and is a drift guard, not a
  measure of third-party coverage. These 189 packs are outside it.
- **Legacy tags are carried, not interpreted.** 115,333 of them. They are
  recognised so the source is not lost, but they carry no mechanics.
- **Non-d20 systems are transcribed too** (2e, 4e, 5e, Star Wars, Traveller,
  Talislanta). The canonical format is system-agnostic; whether the engine has
  anything useful to do with a 5e pack is a separate question.
- **A clean round-trip means the pack is readable, not that it is right.** The
  differential oracle and the deep-field oracle still cover only the RSRD/SRD
  cases they were built for.
