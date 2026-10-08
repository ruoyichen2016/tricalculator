# Inventory: where PCGen syntax still lives

The question this answers, for every field that still carries PCGen text:

1. **Who evaluates it?** — if nobody, structuring it buys nothing.
2. **Who needs the original spelling?** — if the string is displayed, hashed, or
   used as an identity, structuring it *breaks* something.

Two Stage 2 steps were abandoned for failing these questions (`ClassGrant.guards`)
and for failing question 2 while looking cosmetic (`entities.name`). This is the
measurement that should have come first.

Measured on the RSRD basics pack (`_rsrd_basics.pcc`), after Stage 2 step 1.
Counts are occurrences of a field+marker pair, so one field can appear once per
kind of PCGen syntax it carries.

## The table

| Field | Count | Read by engine? | Interprets the syntax? | Verdict |
|---|---:|---|---|---|
| kit `entries.conditions` | 192 | no | no | carried only |
| `entities.name` / `abilities.*.name` | 46 | **yes** (class matching) | no — used as an identity | **keep the spelling** |
| `classes.*.spell_levels` | 44 | no | no | ✅ deleted as a duplicate, see §3c |
| `entities.legacy_tags` | 39 | no | no | reporting metadata, by design |
| `classes.*.level_prerequisites` | 12 | no | no | carried only |
| `traits.damage_reduction` | 16 | yes | **was no — fixed, see §3b** | ✅ fixed |
| `classes.*.prohibited_spells` | 16 | **now** | **now** | ✅ built, see §5b |
| `entities.metadata.added_domains` | 4 | **yes** | **YES** | ✅ fixed, see §6 |
| `ClassGrant.guards` | 3 | no | no | needs the spelling to render |
| `entities.effects.action.tag` | 2 | yes | no | see note below |
| `deities.*.domains` | 2 | validation only | no | carried only |

## Findings

### 1. Almost nothing is evaluated

`spell_levels`, `level_prerequisites`, `prohibited_spells`, kit
`entries.conditions` and `legacy_tags` have **zero readers anywhere in the
workspace**. They are parsed out of the `.lst` files, stored, persisted, and
never consulted.

This is worth stating plainly because it inverts the intuition: these fields look
like rules (`prohibited_spells` decides what a Wizard cannot cast), but nothing
acts on them. Structuring them would be motion, not progress. Whether the
*engine* should honour them is a separate, larger question — that is missing
behaviour, not a data-shape problem.

### 2. Being read is not the same as being interpreted

`traits.damage_reduction` is read by the engine — 12 references — but only
collected, sorted, deduped and emitted. The condition inside `5/Magic|PRELEVEL:MIN=4`
is carried as opaque text and **never evaluated**. The engine reads the field
without reading the syntax.

### 3. One field is genuinely parsed, and it is the last one

`metadata.added_domains` reaches `resolve_added_domain`, which hand-parses PCGen
syntax inside the engine:

```rust
let (domain, condition) = match raw.split_once('[') { ... };
if let Some(body) = condition
    .strip_prefix("PRETEMPLATE:")
    .or_else(|| condition.strip_prefix("pretemplate:"))   // lowercase fallback
```

The lowercase fallback is the signature of hand-rolled parsing — it exists
because someone hit a spelling the first branch missed. This is exactly what
Stage B removed from `engine.rs` and `effects.rs` for formula syntax.

**`PRETEMPLATE:` at `engine.rs:2436` is the only PCGen tag literal in the entire
engine.** So this is an isolated leftover, not the tip of a larger problem.

### 3a. Resolved

`added_domains` is now `Vec<AddedDomain>`, with the condition lowered into a
`Predicate` by `parse_added_domain`. `resolve_added_domain` is deleted and the
engine evaluates the condition through `predicate_holds`, the same function that
evaluates every other condition.

Widening the guard to tag syntax then turned up a **second** live case the
enumeration had missed: `mastervar("X")` in `evaluate_companion_formula`, which
normalised a PCGen formula with a regex *at evaluation time*. That is formula
syntax — the category the guard claimed to cover — and it was invisible because
the token list enumerated only the tokens that had already bitten. `mastervar`
is now a vocabulary term (`pcgen_core::formula_vocab::MASTER_LEVEL`), rewritten
at lowering like `CL` and `classlevel`.

With both gone the engine contains **no PCGen syntax at all**, and `regex` is no
longer used anywhere in the workspace — 87 packages in `Cargo.lock` became 83.

### 3b. Resolved: the gates inside `damage_reduction` were never evaluated

`traits.damage_reduction` was read by the engine, which is why it looked fine.
But being read is not being interpreted: the entries were collected, sorted,
deduped and emitted with their conditions intact.

A level-1 Celestial's snapshot was therefore:

```
["10/Magic|PRELEVEL:MIN=12", "5/Magic|PRELEVEL:MIN=4"]
```

Neither applies below level 4. Both were reported, and the raw PCGen text
reached the character sheet. `10/silver|PRETEMPLATE:1,Natural Lycanthrope` had
the same problem: a lycanthrope showed both the natural and the afflicted
reduction.

The entries are now `DamageReduction { value, condition }`, lowered through the
same `parse_predicate`, and the engine evaluates the gate with `predicate_holds`
where it has the draft to evaluate against. The level-1 Celestial is now `[]`,
and an ungated `5/Bludgeoning` still comes through as its value alone.

This is the case that justifies the whole inventory: the field was on the
"carried only" list, and the defect was not in its shape but in a condition
nobody evaluated.

### 3c. Resolved, and it was not a missing gate — it was a dead duplicate

`spell_levels` looked like the DR case: 50 rows of
`CLASS|Cleric=1|Obscuring Mist|...|PREVAREQ:AirDomainAllowed,1` that nothing
read. It is not.

Those rows are **domain spell lists**, and `DomainDefinition::domain_spells`
already carries the same data structurally — and *is* consumed, so clerics do
get their domain spells. Compared across the 22 domains the rows name:

| | count |
|---|---:|
| identical to `domain_spells` | 18 |
| differing only in case | 4 |
| missing from the structured side | 0 |

The four are the `Magic Circle against X` spells. `rsrd_classes.lst` (the class's
`SPELLLEVEL` rows) spells them with a capital `Against`; `rsrd_domains.lst` and
`rsrd_spells.lst` — the spell's own definition — use lowercase. So the raw data
disagrees with itself, and the duplicate carried the wrong variant.

The field is deleted. The 44 rows were a second copy of data that was already
consumed, one that no code read and that a lookup would have missed on.

Not every "never read" field is missing behaviour. This one was redundant, and
the way to tell was to compare it against the field that *is* read.

### 4. The architecture guard does not cover it

`scripts/check_architecture.sh` check 5 used to guard *formula* syntax only:

```
engine_tokens='classlevel|APPLIEDAS|"VAR\.|\bCL\b|\bTL\b'
```

Tag syntax such as `PRETEMPLATE:` passed straight through. The guard was written
against the two quiet defects that had actually bitten (`BONUS:VAR|A,B|...`, `CL`
resolving to 0), so it was narrow by construction — which meant the principle
"the engine does not interpret PCGen text" was enforced only where it had
already been violated.

It now also matches `mastervar` and quoted tag syntax (`"PRE[A-Z]+:`,
`"ADDDOMAINS`, `"BONUS:`, `"SPELLLEVEL`, `"ABILITY:`). Verified to have teeth:
injecting `PRETEMPLATE:` into the engine makes it fail.

### 5. Resolved: `GrantTag` carried a gate as if it were a value

`AUTO:WEAPONPROF|TYPE=Martial|PREHD:MAX=0` was split on `|` and every part
inserted as a weapon proficiency name, so `TYPE=Martial` and `PREHD:MAX=0` both
became proficiencies and the Hit Dice gate was never evaluated. A 12th-level
character was granted the Outsider house rule's martial proficiency.

`lower_grant` now takes the predicate out of the row and makes it the effect's
`condition`, where the existing evaluator sees it, and only real values reach
the tag. Verified with a control: the ungated row grants `type.martial` at any
level, the gated row grants it at 0 HD and not at 12, and `prehd` no longer
appears among the granted names.

**This is the second instance of the same mistake**, after
`damage_reduction` — a predicate sitting inside a value list, treated as data.
Both were found by asking whether a field that is read is actually *acted on*,
rather than by looking at the field's shape.

### 5a. "Never read" has three causes, and they need different answers

This started as "who evaluates it". Checking each zero-reader field against the
field that *is* read split them three ways, and only one of them is a defect:

| Field | Cause | Answer |
|---|---|---|
| `classes.*.spell_levels` | redundant | deleted — `domain_spells` has it and is consumed (see §3c) |
| `traits.damage_reduction` | gate not evaluated | fixed (see §3b) |
| `classes.*.prohibited_spells` | **half a feature** | see below |
| `classes.*.level_prerequisites` | redundant | the real prerequisites are on the entity's `requirements`, which is consumed |
| kit `entries.conditions` | out of scope | `kits` is imported for counting only; the builder does not apply kits by design (`M9_PERSONAL_BUILDER.md`) |

**`prohibited_spells` was the one genuine gap, and is now built.**

`ProhibitedSpell { target, condition }` splits `SCHOOL.Abjuration` from its
`PREVARGTEQ:ProhibitAbjuration,1` gate, and the engine reports the result as
`prohibited_schools` / `prohibited_alignments` on the snapshot. The Web spell
picker passes them back as `exclude_school` / `exclude_alignment` and filters.

Two things had to be added along the way:

- **`Predicate::DeityAlignment`** for `PREDEITYALIGN`. The Cleric entries are
  `ALIGNMENT.Good|PREDEITYALIGN:LE,NE,CE`, and the deity's alignment is not the
  character's — a Cleric may be one step from their deity, so reusing
  `Predicate::Alignment` would have been wrong.
- **`SpellDefinition::descriptors`** for `DESCRIPTOR:`. The alignment half of
  the feature is unenforceable without it, and it was not captured at all.

The original finding, for the record: The raw data implements the
feature in two halves:

```
Forbid Abjuration   CATEGORY:Special Ability  TYPE:WizardProhibitSchool
                    DEFINE:ProhibitAbjuration|0   BONUS:VAR|ProhibitAbjuration|1
Wizard              PROHIBITSPELL:SCHOOL.Abjuration|PREVARGTEQ:ProhibitAbjuration,1
```

The first half works: the ability sets the variable. The second half does not
exist — `prohibited_spells` is parsed and never consulted, and the Web spell
picker (`spell_search_options`) filters on class, level, domain, school and
search text, but never on what the character has forbidden. A Wizard who forbids
Abjuration is still offered Abjuration spells.

That is a feature to build, not a format to change, so it is recorded rather
than done here. Note the same table applies to Cleric and Druid, whose entries
are alignment-based (`ALIGNMENT.Good|PREALIGN:LE,NE,CE`), so a Cleric of an evil
deity is likewise still offered Good spells.

### 5b. Sweep: predicates embedded in value lists

The signature of the `damage_reduction` and `AUTO:WEAPONPROF` bug is a string
holding **both** `|` and a `PRE*` predicate, so the predicate is one of the
values. Scanning the whole pack for that signature finds five kinds, all of them
in `entities.legacy_tags`:

| count | field | sample |
|---:|---|---|
| 15 | `SPROP:` + `PRECLASS` | descriptive special-property text |
| 14 | `ASPECT:NAME` + `PREABILITY` | display text |
| 7 | `ASPECT:NAME` + `PREVAREQ` | display text |
| 2 | `COMPANIONLIST` + `PRESIZE` | companion list |
| 1 | `SPROP:` + `PRERACE` | descriptive text |

`legacy_tags` has no readers at all, and the two that could have mattered do not:

- `ASPECT` is display text, and the engine does not build sheets from it.
- Companion data comes from `pack.companion_mods` (31 keys, 108 rows, 15 types,
  including `Familiar` and `Special Mount`), not from the tag.

So nothing here reaches a character. The `AUTO:WEAPONPROF` fix covered the same
`lower_grant` branch that `ARMORPROF`, `LANG` and `LANGBONUS` use, so those are
covered too.

### 5c. Sweep: `FOLLOWERADJUSTMENT`, modelled nowhere but unreachable

`COMPANIONLIST:Animal Companion|Companion ~ Bear (Brown),...|FOLLOWERADJUSTMENT:-6`
carries a real rule: a druid taking a Brown Bear treats their level as six lower.
The signature scan above missed these because they hold `|` but no predicate.

`FOLLOWERADJUSTMENT` appears ~59 times in 35e, including the RSRD basics, and is
**modeled nowhere** — the engine matches companion rows by `master_level` alone.
A 1st-level druid could claim a tyrannosaurus and nothing would object.

It is not a defect, though, because the feature has no entry point:
`draft.companions` is populated by no Web route, no CLI flag and no `.pcg`
import — only by tests. Companion *selection* was never built, so the missing
constraint is not yet reachable.

That makes it a scope question rather than a bug, and it is recorded as one.

## What followed from this

Both actions are done:

- `added_domains` is structured at lowering; `resolve_added_domain` is deleted
- the guard covers tag syntax, and widening it immediately found `mastervar`

The engine now contains no PCGen syntax, and `regex` — whose only remaining use
was that `mastervar` regex — is gone from the workspace entirely.

Everything else on the table is either carried-only data (structuring buys
nothing) or load-bearing spelling (`entities.name` — see `effects.rs:787` and the
note there about the 2-point `skill_total` regression).

The larger question this inventory exposes is not about data shape at all: five
fields that encode real rules are parsed and then ignored. That is a scope
question, and it belongs in `PRODUCT_SCOPE.md`, not in Stage 2.
