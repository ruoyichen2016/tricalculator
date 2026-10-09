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

`CompanionModDefinition::abilities` was a candidate for the same embedded-value
bug — the engine pushes it into the snapshot verbatim — but the 22 rows are
already clean names (`Familiar ~ Empathic Link`, `Animal Companion ~ Evasion`),
and a companion-bearing snapshot carries no PCGen syntax either.

### 5d. Where the sweep stands

No PCGen syntax reaches a character snapshot on any path reachable today:
Fighter, Wizard, Cleric, Rogue, Druid and Paladin at several levels; the
Celestial, Lycanthrope, Skeleton and Vampire templates; and Animal Companion,
Familiar and Special Mount. The remaining PCGen text in the pack is confined to
`legacy_tags`, which has no readers, and to fields the inventory has classified
and accounted for.

### 5e. Sweep: the remaining `|`-stored fields

23 fields hold `|`-separated text. Checking each for how it is consumed found two
more defects and two more scope gaps.

**`HITDIE:` was ignored whenever it had a suffix or a `%`.** The raw data spells
a Hit Die override three ways — `HITDIE:10`, `HITDIE:%/12` (set) and
`HITDIE:%up1` (one step up) — and appends a `|CLASS.TYPE=Monster` scope. The
parser stripped neither, so four values silently returned `None` and the
override did nothing; `8|CLASS.TYPE=Monster` is plainly an 8. Lowering also
stores these with a `d` in front (`d%/12`), so the `d` has to come off before the
`%` is even recognisable — I got that order wrong on the first attempt and the
`%` forms still did nothing.

**`NATURALATTACKS` reached the character sheet.** The sheet printed the joined
raw rows, so a wolf's attack routine read
`Bite,Weapon.Natural.Weapon Group Natural.Melee.Finesseable.Bludgeoning.Piercing.Slashing,*1,1d6`.
The tag also packs several attacks into one value with `|`, and reading it as a
single attack let the second attack's fields overwrite the first's — a bear came
out with one claw at one die instead of two claws and a bite. Now
`NaturalAttack { name, count, damage, types }`, rendered as `Claw ×2 1d8`.

**Two fields point at behaviour that was never built**, so they are scope gaps
rather than defects:

- `unencumbered_move` (the Dwarf's "full speed in heavy armour") has no readers —
  because the engine has no encumbrance or armour speed penalty at all. `负重` is
  listed as in scope in `M9_PERSONAL_BUILDER.md`, and carrying capacity is
  likewise absent.
- `exchange_level` was passed to the snapshot as a raw string
  (`Ex Paladin|11|10|1`), the same shape of leak as `NATURALATTACKS`. Now
  structured — see §5f.

### 5f. PCGen ships its own tag reference, and it corrected two of my guesses

`raw/pcgen-master/docs/listfilepages/datafilestagpages/datafilesclasses.html`
is the authoritative spec for these tags. It is worth reading **before**
implementing rather than after, which is what I did here.

**`HITDIE` — my `%/N` was wrong.** The reference:

```text
HITDIE:x
  x: Number        Hit Die size
  x: %+Number      added to the Hit Die
  x: %-Number      subtracted from the Hit Die
  x: %*Number      Hit Die is multiplied by
  x: %/Number      Hit Die is divided by
  x: %upNumber     stepped up by
  x: %downNumber   stepped down by
  x: %HupNumber / %HdownNumber   the same
```

`%/N` is **division**, not "set to N". I had read `%/8` on the `Eighth Hitdie`
template as "the die is 8" and returned `Fixed(8)`, which left a d8 monster
exactly as it was. It is the fractional-Hit-Die mechanic: a d8 becomes a d1. The
other four operators (`%+`, `%-`, `%*`) were not handled at all, and `%Hup` /
`%Hdown` are alternative spellings I had also missed.

**`EXCHANGELEVEL` — the four numbers now have names.** The reference:

```text
EXCHANGELEVEL:w|x|y|z
  w: Class Name
  x: Class level, minimum level required in donating class
  y: Class level, maximum levels donated from class
  z: Class level, lowest that donation can lower donating class level to

EXCHANGELEVEL:Ex Paladin|11|10|1
  "Up to 10 levels of Ex Paladin can be exchanged as long as there were at
   least 11 levels of Ex Paladin available."
```

So the value is `ExchangeLevel { donating_class, minimum_level, maximum_levels,
floor_level }`, not an opaque string. Without the reference I would have had to
guess, and the honest options were a guessed name or no name at all.

**The lesson**: this repository vendors PCGen's documentation, so for any tag
whose semantics are not obvious, the spec is one `grep` away. Both of these had
been guessed at — one wrongly.

The rest were checked and are sound: `proficiency` (`SHIELD|Shield (Heavy)`) is
read via `starts_with`, `hit_die_override` now parses, `natural_attacks` is
structured, `spec` / `choose` / `tag` go through the choice and grant machinery,
and `legacy_tags`, `name` and `key` are accounted for above.

### 5g. Auditing my own implementations against the vendored spec

Having found the reference, the useful move was to re-read everything I had
implemented from inference. Three of them were wrong.

**`PROHIBITSPELL` handles five target kinds, not two.**

```text
PROHIBITSPELL:x
  ALIGNMENT.Text   DESCRIPTOR.Text   SCHOOL.Text
  SUBSCHOOL.Text   SPELL.Text

Alignment, descriptor, school and subschool names are period-delimited lists.
Spell name is a comma-delimited list.

PROHIBITSPELL:ALIGNMENT.Chaotic.Evil
  Prohibits spells that have both the Chaotic and the Evil descriptors.
```

I modelled `SCHOOL` and `ALIGNMENT` as single names. That silently dropped 23
`SPELL.*` and 11 `DESCRIPTOR.*` entries — every one of them in Unearthed Arcana
and other non-RSRD data, which is precisely the data that is meant to be usable.
The RSRD exercises none of them, so the acceptance gate could not have caught it.

Two further consequences of reading the spec properly:

- A multi-name target is a **conjunction** (`Chaotic` *and* `Evil`), so the
  snapshot keeps the structured entries. My first version flattened them to a set
  of names, which cannot express "both".
- `SUBSCHOOL` was not captured at all, and the RSRD spells file has 228
  `SUBSCHOOL:` tags.

**`ADDDOMAINS` is `x|x`, a list.** I read the whole value as one domain, so
`ADDDOMAINS:Travel|Sun` produced a domain literally named `Travel|Sun` that
matched nothing — neither domain arrived. The `[...]` qualifies the domain
immediately before it, which the tests now pin down: `Sun[PRETEMPLATE:...]|Law`
keeps the gate on Sun and leaves Law unconditional.

**Still open, and a semantic rather than syntactic question:** the spec says
ADDDOMAINS "does not add them to the character's domains, but merely adds them
to the list they can choose from". The engine grants the domain's spells
outright, so a vampire cleric receives the spells of all four domains it may
choose between. That is pre-existing behaviour, not something the restructuring
introduced, and changing it needs a domain picker — recorded, not fixed.

**Also unsupported:** `PREDIETY`, which the reference uses in an ADDDOMAINS
example. No data uses it, so it is noted rather than added.

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
