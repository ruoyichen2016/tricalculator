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

### 5h. The audit, continued: four more tags guessed at

**`DR` — same-type reductions stack in our output but must not.**

```text
DR:x/y
  x: Number or Formula (Amount of Damage Reduction)
  y: Text (Damage Type that bypasses this Reduction)
  Prerequisites Allowed: Yes

"DR tags with the same damage types do not stack, only the DR tag with the
 highest value will be used."
```

A 12th-level Celestial carries `5/Magic` (gated at level 4) and `10/Magic`
(gated at level 12). Both gates are met, and the character was reported as having
both. Now collapsed to one entry per bypass type, keeping the highest; entries
whose amount is a formula are left alone rather than compared wrongly.

**`PREWEAPONPROF` — a documented type requirement was dropped.**

```text
PREWEAPONPROF:x,y,y
  y: Text (Weapon proficiency name)   y: TYPE.Text (Weapon Proficiency type)
```

`parse_qualified_names` filtered `TYPE=` out, which emptied the list and made the
caller discard the whole predicate — so `PREWEAPONPROF:1,TYPE=Martial` required
nothing at all. Five RSRD prerequisites are affected. Both spellings (`TYPE=` in
the data, `TYPE.` in the reference) now normalise to the `type.<kind>` marker the
granting side already wrote.

**`PRERACE` — one of five spellings, and the one implemented was exact.**

```text
PRERACE:1,Dwarf,Elf,Human              names
PRERACE:1,Elf%,[Elf (aquatic)]         the % wildcard
PRERACE:1,TYPE=Dire                    a race TYPE tag
PRERACE:1,RACETYPE=Giant               the creature type
PRERACE:1,RACESUBTYPE=Incorporeal      a race subtype
PRERACE:2,RACETYPE=Undead,RACESUBTYPE=Incorporeal
```

Measured against the RSRD, **1,141 of 2,131 values (54%) could never match**:
929 bare `%` (which the reference's own example defines as matching any race),
82 `RACETYPE=`, 56 `RACESUBTYPE=`, 40 prefix patterns, 34 `TYPE=`. All five now
lower to a `RaceSpec`, and `RACETYPE` — used 26 times in the RSRD races and not
captured at all — is lowered too.

**`NATURALATTACKS` has a fifth, optional parameter.** The reference gives
`v,w.w,x,y,z` with `z: SPROP=Text`. Filed as a weapon type, it polluted the type
list; 222 rows carry one.

**`MOVECLONE` and `MOVE` were already right**, checked rather than assumed:
`MOVECLONE:Walk,Fly,*2` gives fly 60 from walk 30, and `MOVE:Walk,30,Fly,10` is
the flat pair list the code expects.

### 5i. A trap worth recording: the gate was right and never reached

Adding `RaceMatches` made the deep-field oracle fail on `3e_BarJack` and
`3e_SWizSam`, `skill_total` 20 against Java's 22. The new gate was correct — the
characters were losing the Gnome racial traits.

`internal_race_selector` is what activates PCGen's racial-defaults entity, and it
asks whether any of the entity's grants is gated on a race by calling
`contains_race_predicate`. That match had a `_ => false` arm, so a `RaceMatches`
condition was not recognised, the entity was never activated, and the correct
gate was never evaluated.

Two lessons, both about how I verified things:

- The trace that settled it (`race_spec_matches` never being called) was worth
  more than any amount of reading. I had already checked the parsing was right
  and the evaluation arm was in the right function; both were true, and neither
  was where the fault lay.
- My first three test harnesses for `PRERACE` proved nothing — `weapon_proficiencies`
  is not on the snapshot, `active_abilities` holds only *granted* abilities, and
  a selected ability is not requirement-checked at all (`grant_allowed` is called
  from one place, the grant path). Only the fourth, which observed a granted
  ability's side effect, could fail.

### 5j. `PRESKILL`, `PREABILITY` and `PREMULT`

**`PRESKILL` read only the last pair, and did not know `TYPE.`.**

```text
PRESKILL:x,y=z,y=z
  x: Number (Number of skills)
  y: Text (Skill name)   y: TYPE.Text (Skill type)
  z: Number (Skill ranks)

PRESKILL:1,Spot=10,Listen=10   either, with at least 10 ranks
PRESKILL:2,TYPE.Spy=2,TYPE.Spy=2
  "two Spy skills with at least two ranks in each of them"
```

507 of the RSRD's 1,626 values list more than one pair (or a count above one),
and only the last was tested; 204 use `TYPE.`, which looked for a skill literally
named `TYPE.Perform`. `SkillDefinition` also had no `TYPE:` tags — the thing a
type requirement matches — so those are captured now.

**`PREABILITY` had three separate faults.**

```text
PREABILITY:1,Category=Feat,TYPE.SpellFocus    one SpellFocus feat
PREABILITY:3,Category=Feat,TYPE.Metamagic     three Metamagic feats
PREABILITY:1,Category=SpecialQuality,[Night Vision]
  "Requires that the creature does not have the Night Vision special quality"
PREABILITY:1,CATEGORY=Feat,Power Attack,[Weapon Finesse]
  "has Power Attack, and does not have Weapon Finesse"
```

- Only the **last bare name** was read, so of the RSRD's 7,183 values, the 2,481
  with several names tested just one.
- `TYPE.` was not understood at all — 1,303 values.
- A **bracketed name is an exclusion**, and 924 values use one. Taken as a
  requirement it never matched, so those gates were permanently shut.

`CompiledEntity` carried no `TYPE:` tags, so ability types had to be captured
before any of this could work.

**The count means something different here.** For `PRESKILL` it is the number of
listed specs satisfied (the reference says to repeat a `TYPE.` sub-tag once per
instance, since each carries its own rank threshold). For `PREABILITY` it is the
number of **held abilities** matching any listed ref — `PREABILITY:3,...,
TYPE.Metamagic` lists one type and asks for three feats carrying it. My first
attempt used the `PRESKILL` reading and would have failed that case; the tests
pinned it down.

**A trap the gate caught, not me.** Handling only `TYPE.Metamagic` made four RSRD
values unparseable, and they surfaced in the unsupported-tag report rather than
in any behavioural test — the pf35e gate failed on
`unsupported_pending_by_tag = {'PREABILITY': 4, 'PREMULT': 2}`. The RSRD writes
`TYPE=Metamagic`; the reference's examples write `TYPE.Metamagic`. Both spellings
are handled now, as they already were for `PRERACE` and `PREWEAPONPROF`.

### 5k. `BONUS` has four types, and `.REPLACE` was wrong twice over

```text
BONUS:<category>|<group>|<formula>   then |PRExxx or |TYPE=<type>

Typeless   no TYPE=            always added
Standard   TYPE=name           only the highest of the same type counts
.REPLACE   TYPE=name.REPLACE   counted together, then compared with the
                               standards; the highest wins
.STACK     TYPE=name.STACK     stacks with itself and is added on top
```

The reference gives worked examples, and both `.REPLACE` ones failed:

- `.REPLACE 3 + 2` against standards `4 + 2` → 5
- `.REPLACE 1 + 1 + 2` against standards `2 + 6` → 6

Two faults. `.REPLACE` was grouped apart from the standard type of the same name,
so `luck.replace` and `luck` were **added** rather than compared; and it took the
highest single value rather than the sum. `.STACK` and the typeless case were
already right.

The grouping now strips `.REPLACE` / `.STACK` so the competing bonuses share a
group, and a small `BonusGroup` accumulates the highest standard, the sum of the
replacements and the sum of the stacks, then combines them the way the reference
describes. Both call sites — the general component split and the AC one — use it.

### 5l. `PRECLASS` and `DEFINE`

**`PRECLASS` read only the first pair, and knew one of four ways to name a class.**

```text
PRECLASS:x,y=z,y=z
  x: Number (The number of classes that must be equal to or greater than the
             numbers specified for the check to succeed)
  y: Text (Class Name)   y: TYPE.Text (Class Type)
  y: SPELLCASTER   y: SPELLCASTER.Type (Spellcaster Type)   y: ANY
  z: Number (Class Level)

PRECLASS:2,Wizard=5,Sorcerer=6,Cleric=7
  "Multi-classed character must be at least Wiz5/Sor6, Wiz5/Clr7 or Sor6/Clr7"
PRECLASS:1,SPELLCASTER=2      two levels in any spellcasting class
PRECLASS:1,SPELLCASTER.Arcane=2
```

The count was ignored, so 140 of the RSRD's 1,735 values tested a single class.
`SPELLCASTER` (332 values), `TYPE.` (51) and `ANY` (2) each became a requirement
for a class *by that literal name*, which cannot exist — 385 values that could
never be satisfied.

`SPELLCASTER` sums the levels of every matching class, which is how a multiclass
caster is treated: a Sorcerer 6 / Wizard 6 has twelve spellcaster levels.

`Predicate::ClassLevel` lost its only producer and was removed. Its `max` had
never been meaningful: the reference gives `z` as a plain level and no data uses
a range form.

**`DEFINE` kept the last definition rather than the highest.**

> Values are calculated by getting the value of the variable with the **highest
> defined value** … If it finds more than one, it **keeps the one with the higher
> value**.

The definitions went into a map keyed by name, so the winner depended on
iteration order. They are now collected per name and the highest evaluated value
wins, with a definition that fails to evaluate reported and skipped rather than
discarding the whole variable.

### 5m. Patch order, and a test that asserted a fiction

PCGen's file-format notes give the order explicitly:

```text
The order of operations is: Native line, COPY, MOD, then FORGET, but the order
of operation is also controlled by the RANK tag in the PCC file.

Given:  1.MOD  1.FORGET  2.COPY  2.Blank  2.MOD  3.MOD
Order:  2.Blank  2.COPY  1.MOD  2.MOD  3.MOD  1.FORGET
```

**`.FORGET` never worked.** The pass looks the record up by `RawLstLine::key()`,
which strips `.MOD` and everything from `.COPY=` on — but not `.FORGET`. So
`Fighter.FORGET` keyed as `fighter.forget` and never matched `fighter`.

What hid it was the test. `patched.rs`'s test helper built its key as
`name.split('.').next()`, which the parser never produces, so the test passed
against a key that cannot occur. Replacing it with the real rule made the
built-in test fail immediately, which is how the fault surfaced.

**A `.MOD` before its base was dropped.** The pass is one stream-order sweep, so
a MOD arriving before the native line found no base and was pushed through
unmerged. The order above says a MOD applies *after* the native line however the
two are ordered, so deferred MODs are now held and applied at the end.

`.COPY=` was **already correct**, and I broke it before checking: the lookup uses
`key()`, which is the base name, so a "fix" to derive the base from the record
name myself was both unnecessary and wrong for the `CATEGORY=Feat|X.COPY=Y`
spelling. Reverted. Worth recording because the mistake was to change working
code on the strength of a test whose helper was itself unrepresentative.

### 5n. `NUMCHOICES` / `SELECT` — a constraint, not a parse

`CHOOSE:NUMCHOICES=3|SKILLSNAMED|Spot|Listen|Search` caps the total selections a
chooser may take; `SELECT:n` sets how many are taken at once. `NUMCHOICES` has no
readers at all (119 uses in 35e) and `SELECT` is handled only for kits, so the
spec string becomes an opaque key in `draft.choices` and the cap is never
enforced.

Recorded rather than built: no `.pcg` in the vendored test data carries a
`CHOICE:` line, so nothing in the acceptance cohort exercises a chooser and there
is no measurable behaviour to correct. This is a missing constraint, like
`FOLLOWERADJUSTMENT`, not a defect.

### 5o. A coverage sweep instead of another tag at a time

Rather than reading one more spec, ask the importer what it cannot account for.
The RSRD basics and the 3e SRD report **zero** unsupported and zero legacy tags;
UA reported four `BONUS` rows and the third-party packs a handful each.

**`PREGENDER` had no data behind it.** The four UA `BONUS` rows were
`BONUS:PCLEVEL|Cleric|1|PREGENDER:F`, and `PREGENDER` was not parsed. But the
gap was deeper than the predicate: `PREGENDER` has 60 uses in the vendored data,
**40 of the 42 vendored `.pcg` files carry a `GENDER:` line**, and the character
model had no gender field at all — so the import dropped it and the requirement
could never be evaluated either way.

The reference: *"The character's gender must start with the text specified. The
test is case sensitive so \"male\" is not the same as \"Male\"."* That prefix
comparison and its case sensitivity are what the tests pin down.

**`PRESPELLTYPE` was implemented, but not its `ANY`.** The reference lists
`Arcane`, `Divine`, `Psionic` and `ANY` for the second parameter; `ANY` failed
`SpellPowerType::from_pcgen`, so the whole tag became unsupported. A spell's own
power is concrete, so `ANY` belongs to the *requirement*: the predicate's `power`
is now optional and `None` means any.

**A fragility worth naming:** `lower_bonus_effect` reads its qualifiers with
`parse_predicate(part, "")?`, so a single unparseable predicate discards the
whole bonus — the numeric effect is lost because of a gate. That is how four
`BONUS:PCLEVEL` rows disappeared. It is left as it is for now, since the
conservative direction is to withhold a gated bonus rather than grant it
ungated, but a `PRE*` tag we cannot read costs the entire bonus rather than just
its condition.

### 5p. What the sweep leaves

After these fixes the unsupported tags are all in third-party datasets outside
the acceptance cohort: `PRESPELLTYPE`, `PREVISION`, `PREABILITY`, `PREMULT`,
`PRESIZENEQ`, `PRE`, `PRECSKILL`, `PRESPELLDESCRIPTOR`. The legacy-pending ones
(`SAB`, `NAMEISPI`, `DESCISPI`, `FACTSET`, `SPELLKNOWN`, `BONUSSPELLSTAT`) are
recognised-but-unread, which is the intended state for reporting metadata.

### 5q. The acceptance cohort loads the full RSRD, epic included

The deep-field config uses `_rsrd_basics.pcc`, which reports zero unsupported
tags — but the **broad cohort** loads
`data/35e/wizards_of_the_coast/rsrd/_revised_system_reference_document.pcc`, the
full RSRD with the epic pack, and that had four tag kinds unparsed:

| tag | uses | what it needed |
|---|---:|---|
| `PRELEGSLTEQ` | 27 | `LegsAtMost` — the `GTEQ` spelling existed, the `LTEQ` one did not |
| `PRESPELLTYPE` | 11 | every `y=z` pair, not just the first |
| `PRESRGTEQ` | 2 | creature trait data |
| `PREVISION` | 2 | creature trait data |

**`PRESPELLTYPE` takes several pairs.** The reference:
`PRESPELLTYPE:1,Arcane=3,Divine=3,Psionic=3` — "at least one 3rd level spell from
**any** of the Arcane, Divine, or Psionic types". So the pairs are alternatives
and the count applies across their union. `split_once('=')` on the second pair
put `9,Divine=9` into the level, which failed to parse and took the tag with it.
The predicate now holds a list of `SpellTypeSpec`, each with an optional power
(`ANY`) and a minimum level. `PREHANDSLTEQ` and `PRELEGSGTEQ`/`LTEQ` are read
from one branch each.

**Two are left, and deliberately not half-done.** `PRESRGTEQ` (spell resistance)
and `PREVISION` (vision type and range) both need creature traits — which come
from the **race and the active templates together**, a merge that lives in the
engine layer (`merge_creature_traits`) and is not reachable from
`predicate_holds`. A race-only version would have been three lines and would have
answered **wrongly** whenever a template was the source, which is worse than
leaving the tag visibly unsupported. The cohort's unparsed set is down from 4
kinds (42 uses) to 2 kinds (4 uses), and both remaining ones are named.

### 5r. Creature traits at the predicate layer

`PRESRGTEQ` and `PREVISION` were the last two tags the acceptance cohort could
not parse. Both need creature traits — spell resistance and vision — which come
from the **race and the active templates together**. `merge_creature_traits`
does that merge, but it lives in the engine layer and runs *after*
`apply_effects`, so `predicate_holds` could not reach it.

Neither merge needs a gate evaluated (`damage_reduction` does, which is why that
one has to stay where the state already exists), so both now happen in
`apply_effects` itself, before the effect loop. `EffectState` carries the vision
entries and the highest spell resistance.

**A vision entry carries its distance in a variable.** `VISION:Darkvision
(DarkvisionRange)` names the variable in parentheses, and the two sides do not
always spell the kind the same way — the data has `VISION:Low-Light` but
`PREVISION:1,Low-Light Vision=1`. The comparison ignores case, spaces and
hyphens and accepts containment either way.

**`DEFINE` had to move earlier.** `PREVISION:1,Darkvision=120` is evaluated while
grants are still being applied, but `resolve_variables` runs after the effect
loop — so the range variable was not yet known and the distance always read as
zero. A pre-pass over the initially-selected entities now collects their `DEFINE`
tags and resolves them first. That is a general improvement: any predicate that
reads a variable was previously answered against a table that was still empty.

The cohort's unparsed set is now **zero**, down from four kinds (42 uses) two
commits ago. What remains is `legacy_pending`: `VISION` and `BONUSSPELLSTAT`,
which are recognised-but-unread reporting metadata.

### 5s. Third-party data, audited by bisection

The acceptance cohort is the full RSRD, which is now fully accounted for. Every
other publisher's data in `raw/pcgen-master` is *outside* that cohort, so nothing
in the gate protects it. A sweep of all 35e/3e packs found 27 unsupported tag
kinds — but the more useful result is that **eight of them are tags this crate
already implements**, failing on a value form the cohort never exercises.

The method matters: `import-report` names the tag but not the value, so each
candidate was bisected by writing every distinct value into a one-record probe
pack and asking which one the importer rejects. That turns "BONUS is
unsupported" into an exact string and an exact cause.

| Tag | Failing value | Cause |
|---|---|---|
| `PREABILITY` | `1,CATEGORY=FEAT,Weapon Focus (TYPE=Bow)` | `trimmed.contains('=')` skips any part containing `=`, so an ability *name* with a `TYPE=` inside parentheses was skipped and `wanted` emptied |
| `PRESKILL` | `1,TYPE=Perform=8` | `split_once('=')` splits at the first `=`, leaving ranks `Perform=8`; and only `TYPE.` is stripped, not `TYPE=`, though `strip_type_prefix` already handles both |
| `PRECLASS` | `1,SPELLCASTER.Divine=9th` | `level.parse::<u32>().ok()?` inside `filter_map` drops the whole spec on free text (`9th`, `9th+`, `level 7`, `1st+ level.`) |
| `PRECLASS` | `1,%` | `=level` is treated as mandatory; the reference makes the level optional, defaulting to 1 |
| `BONUS` | `VAR\|AlchemyFeat\|1\|PRESKILLTOT:Alchemy=7\|TYPE=NoStack` | the gate `PRESKILLTOT` is unimplemented; `lower_bonus_effect` now retains the bonus with a `Predicate::Unsupported` condition and still reports the raw gate |
| `PREITEM` | `1,Manual (Climb)` | only the `TYPE=` form is implemented; all 28 Darwin's World values are names |
| `TEMPBONUS` | `PC\|SKILL\|Diplomacy\|4,PC\|SAVE\|Will\|-1\|PRETEXT:...` | `PRETEXT` is unimplemented, and it poisons the whole bonus |
| `TEMPVALUE` | `MIN=1\|MAX=MaxSpellChanneling\|TITLE=...` | `MAX=` may name a *variable*, not a literal |
| `PREMULT` | `1,[!PRECLASS:1,%],[PREVARGTEQ:EclipseClass,1]` | inherits the `PRECLASS:1,%` bug above |

Two of these are worse than a parse gap. A `PRECLASS` spec dropped by
`filter_map` leaves the *other* specs in place with `min_count` unchanged, so
`PRECLASS:2,Fighter=5,Wizard=9th` silently becomes "two of [Fighter=5]" — an
unsatisfiable predicate rather than a visibly unsupported one. And a `BONUS`
whose gate cannot be read is discarded entirely, so the character quietly loses
a bonus it should have.

The remaining kinds are genuinely unimplemented: `PRE` (21), `PRESKILLTOT`,
`PREHP`, `PRESIZENEQ`, `PREDEITYDOMAIN`, `PRESRLT`, `PRECLASSLEVELMAX`,
`PRETEXT`. None appear in the acceptance cohort.  `PRECAMPAIGN`,
`PRECHARACTERTYPE`, `PRESA` and `PREFACT` were on this list and are now lowered —
see §5aa, §5ab, §5ac and §5ad.  Several names on the original list have since been lowered:
`PRELEVELMAX`, `PREATT`, `PREHANDSGT`, multi-mode `PREMOVE`, `PREREGION`
(see §5u), `PRECSKILL` (see §5v), `PREALIGN`'s value forms (see §5w),
`PRECHECK` (see §5x), `PRESPELLDESCRIPTOR` (see §5y), `PRESPELLSCHOOLSUB` and
`PRESPELLSCHOOL` (see §5z), `PRECAMPAIGN` (see §5aa), `PRECHARACTERTYPE`
(see §5ab), `PRESA` (see §5ac), `PREFACT` (see §5ad), `PREDR` (see §5ae), the
`PREVAR*` boundary (see §5af), `PREREACH` (see §5ag), `PREITEM` (see §5ah),
`PRECLASSLEVELMAX` (see §5ai), `PRESKILLTOT` (see §5aj), `PREUATT` (see §5ak),
`PRESPELLCAST` (see §5al), `PRESPELLTYPE` (see §5am), the `PRELEGS` family
(see §5an) and the `PREEQUIP*` keys (see §5ao);
`PRELEVEL` now supports both `MIN=` and `MAX=`.  See the current `CHANGELOG.md`.

### 5t. The reference parser, and three §5s diagnoses corrected

§5s bisected the third-party failures to exact values, but it had only the
*values*. Reading PCGen's own parser settled what they mean — and refuted three of
the causes I had written down there.

**One rule explains every one of these forms.** `AbstractPrerequisiteListParser`
splits the part on **every** `=`, takes the value from the **last** segment, and
rebuilds the key from the remaining segments joined with `=`:

```java
String[] tokens = thisElement.split("=");
int valueIdx = tokens.length - 1;
min = Integer.parseInt(tokens[valueIdx]);
subreq.setKey(getRequirementKey(tokens));   // tokens[0..n-1] joined with "="
```

and when that last segment is *not* a number, the whole element becomes the key and
the value falls back to the count (`subreq.setOperand(elements[0])`). So:

| Value | Key | Value |
|---|---|---|
| `Spot=10` | `Spot` | 10 |
| `TYPE.Perform=8` | `TYPE.Perform` | 8 |
| `Knowledge (TYPE=Other)=3` | `Knowledge (TYPE=Other)` | 3 |
| `TYPE=Perform=8` | `TYPE=Perform` | 8 |
| `TYPE=Knowledge` | `TYPE=Knowledge` | the count |
| `1,Bard` | `Bard` | the count |

**Correction 1: `PRESKILL` was a split-point bug, not a `TYPE=` bug.** §5s said
"only `TYPE.` is stripped, not `TYPE=`" — but `strip_type_prefix` has always
handled both spellings. The real fault was `split_once('=')`: on `TYPE=Perform=8`
it left the ranks as `Perform=8`, and on `Knowledge (TYPE=Other)=3` it left the name
as `Knowledge (TYPE`. Splitting on the **last** `=` fixes both, and the spec's own
example is the test.

**Correction 2: `PRECLASS:1,%` is not a wildcard and `=9th` is not optionality.**
§5s filed `%` under "the level is optional". `PreClassTester` looks the key up
literally (`character.getClassKeyed(aString)`); there is no `%` handling anywhere,
and a non-numeric level leaves the key as the whole text. Both are malformed data
that the reference itself reduces to nonsense — `SPELLCASTER.Divine=9th` becomes the
key `SPELLCASTER.DIVINE=9TH`. They are now reported as unsupported rather than
transcribed as nonsense.

The real `PRECLASS` gap was the *opposite* of what §5s recorded: **a bare class
name**, `PRECLASS:1,Bard`, which is valid, means "a Bard of at least 1st", and
accounts for all 21 occurrences (the Babylon 5 and Star Wars templates).

**Correction 3: `PRETEXT` is a documented no-op, not an unimplemented tag.** §5s
listed it among "genuinely unimplemented". The reference says:

> This tag in itself does not disqualify anything but alerts the player to the
> additional pre-requisites.

There are no mechanics to implement. The 7,358 occurrences were `PRETEXT` used as a
*gate inside* `BONUS`/`TEMPBONUS`, where `parse_predicate` returned `None` and the
`?` in `lower_bonus_effect` discarded the whole effect. It now lowers to an empty
conjunction — vacuously true — which is exactly "does not disqualify anything".

Across all 393 third-party `.pcc` files this took unsupported tags from **9,409 to
2,035** (−78%), with entities and tag occurrences unchanged.

**One more fault the value list hid: `CHECKMULT`.** 19 `PREABILITY` values read
`1,CHECKMULT,CATEGORY=Feat,Weapon Focus`. `CHECKMULT` is a flag meaning "count
multiple instances of the same ability"; read as a name it became a phantom ability
in the wanted list. It is skipped now. Counting multiples is still not modelled.

**What is genuinely out of scope.** `Weapon Focus (TYPE=Bow)` is not a name with a
stray `=`: the reference splits it into key `Weapon Focus` and a **sub-key**
`TYPE=Bow`, then resolves that sub-key against the ability's association list and
the weapon proficiency's types (`PrerequisiteUtilities.dealWithSubKey`). Nothing in
the engine tracks ability choices or associations, so the text is preserved
verbatim and fails closed rather than silently matching any `Weapon Focus`.

### 5u. `PREREGION`, and the region model it needed

`PREREGION:x` could not be lowered because the character had no region at all.
The tag has two halves that the reference settles, and the second is easy to get
wrong by pattern-matching on the name:

**The region lives on the template, not on the character.** The only
`REGION` token in PCGen is `plugin/lsttokens/template/RegionToken.java`
(`PCTemplate`), with a companion `SUBREGION`; the character's own region is a UI
selection in `RegionFacet`. So the model side is `CreatureTraits.region` /
`.subregion`, parsed in `parse_template` — and `REGION:NONE` is the token's
spelling for "nothing set", *not* a region named `NONE`.

That "template-only" is load-bearing rather than pedantic: the third-party data
writes `REGION:` on RACE and ABILITY rows too, where PCGen has no such token. Those
are not consumed, so they stay visible as legacy instead of being half-honoured by
an engine that reads them where the reference does not. `parse_creature_traits` is
shared by races, templates and companion mods, so the region is deliberately *not*
parsed there; `consumed_legacy_tag` claims the tag for `TEMPLATE` only.

**The test is a case-insensitive prefix of `REGION (SUBREGION)`.** `RegionFacet`
builds `getFullRegion()` by appending `" (SUBREGION)"` when one is set, defaulting
the region to `Region.NONE`, and `PreRegionTester` does:

```java
final String requiredRegion = prereq.getKey().toUpperCase();
final String characterRegion = display.getFullRegion().toUpperCase();
final boolean sameRegion = characterRegion.startsWith(requiredRegion);
```

Three consequences the data depends on:

| Written | Means |
|---|---|
| `PREREGION:Varisia` | region `Varisia (Magnimar)` matches |
| `PREREGION:The` | region `The River Kingdoms` matches |
| `PREREGION:Nidal` on an unset region | fails — `"NONE"` does not start with `"NIDAL"` |

`AbstractPrerequisiteSimpleParser` sets the key to the **whole** formula, so a
comma is not a list separator here: the Arcanis data really does name a region
`Elorii Nations,Dwarven Enclaves`, and only that exact prefix matches. `Q:`
(override-qualify) is stripped before the parser runs, as it is for every other
`PRE` tag.

Lowered to `Predicate::Region { prefix }`; `EffectState.region` is built from the
active templates before the effect loop, the same pass that supplies `VISION` and
spell resistance. This cleared 65 `PREREGION` occurrences **and** 356 `PREMULT`
occurrences, because a `PREMULT` whose inner predicate fails to lower is itself
reported as unsupported.

### 5v. `PRECSKILL`, which is not `PRESKILL`

`PRECSKILL` reads like a sibling of `PRESKILL` and is not one. The reference
says "the number of skills that must be considered **Class Skills**"; the value
has no `=z`, and `PreCSkillParser` is an `AbstractPrerequisiteListParser`, so
`PRECSKILL:1,Spot,Listen` is a count plus names — never ranks.

`PreCSkillTester` asks `character.isClassSkill(skill)`, and that method walks the
character's *classes*:

```java
public boolean isClassSkill(Skill sk) {
    for (PCClass cl : getClassSet()) {
        if (isClassSkill(cl, sk)) { return true; }
    }
    return false;
}
```

So the model side is a class-skill set built from the effective class levels —
the union across classes, plus whatever an archetype or substitution level swaps
in, which PCGen's `class_skills_for_lane` also merges. It is built before the
effect loop, like `VISION` and the base saves.

One lowering wart has to be undone here: `CSKILL:TYPE=Craft` is stored as the bare
text `Craft`, because the class-skill list strips the `TYPE=` marker. A class-skill
entry that names a known skill type therefore expands to every skill of that type,
and the bare entry is kept too — a skill really can be named `Craft`.

**Deliberately not modelled, and worth stating:** PCGen also accepts `CSKILL:` /
`CCSKILL:` *globally* — on templates, races and abilities — through
`GlobalToSkillCostFacet`, and resolves `SERVESAS` skill imitators. Neither is
modelled, and none of the checkout's 30 `PRECSKILL` occurrences needs them: the 12
positive ones name core skills (Handle Animal, Knowledge (Nature), Survival, Ride)
that the classes grant, and the 18 negated ones are 5e, where there is no class
skill concept to consult. `PRECSKILL` is also the
one predicate here whose `TYPE.` selector PCGen counts *per matching skill*
(`runningTotal++` inside the loop) rather than per selector; with a single
selector and a count of 1 — every occurrence in the data — the two agree.

Lowered to `Predicate::ClassSkill { specs, min_count }` with a new `SkillSpec`
selector. `SkillRequirement` was not reused: a `min_ranks` field that means
nothing here is a trap for the next reader.

### 5w. `PREALIGN`, and the two value forms it was quietly mishandling

`PREALIGN` has been implemented since the early PRE work, but two of its value
forms were wrong in opposite directions.

**Numbers were filtered out, which deleted the requirement.** The arm kept only
parts that were non-empty and not parseable as an integer, so
`PREALIGN:0,1,3,4,6,7` produced no predicate at all and the tag was reported
unsupported. That is not a parse failure to be fixed by guessing what `0` means —
`PreAlignParser` performs *no* validation:

```java
String[] alignments = formula.split(",");
if (alignments.length == 1) { prereq.setKey(formula); }
```

and `PreAlignTester` then looks the key up as a `PCAlignment`:

```java
PCAlignment desiredAlign = ...silentlyGetConstructedCDOMObject(PCAlignment.class, desiredAlignIdentifier);
```

No vendored alignment file defines a numeric key — the keys are `LG`..`CE`,
`None` and `Deity`, and the numbers in those files are `SORTKEY`s, which nothing
looks up. So the requirement can never match, and *that* is the faithful outcome.
Dropping it was worse than failing closed: the Book of Erotic Fantasy's Imagist,
Dominator, Disciple of Aaluran and Knot Binder of Kaladis became freely
selectable, with no alignment restriction at all. The numbers are now kept as
ordinary keys, which fail closed.

**`Deity` was taken literally, which made it unsatisfiable.** `alignMatches` has
one special case:

```java
PCAlignment al = getPCAlignment(desiredAlignment);
if (al.equals(charAlignment)) { return true; }
else if (desiredAlignment.equalsIgnoreCase("Deity")) {
    Deity deity = ...DEITYINPUT...;
    CDOMSingleRef<PCAlignment> deityAlign = deity.get(ObjectKey.ALIGNMENT);
    return (deityAlign != null) && charAlignment.equals(deityAlign.get());
}
```

Note the order: the literal comparison comes first, and it is not dead code —
every game mode defines an alignment named `Deity's` with `KEY:Deity`. Only when
that fails does `Deity` mean "the same alignment as the character's god". The
second half was missing, so Complete Divine's *Arcane Disciple*
(`PREALIGN:Deity`) could never be satisfied. `EffectState.deity_alignment` — the
value `PREDEITYALIGN` already needed — answers it, so no new state was required.

### 5x. `PRECHECK`, and the count `PRECHECKBASE` had been losing

`PRECHECKBASE` was implemented early and `PRECHECK` was not, but they are the same
parser — `PreCheckParser` handles both kinds — so reading it settled both:

```java
public int passes(final Prerequisite prereq, final PlayerCharacter character, CDOMObject source) {
    final int operand = character.getVariableValue(prereq.getOperand(), "").intValue();
    final int characterCheckBonus = character.getTotalCheck(check);
    runningTotal = prereq.getOperator().compare(characterCheckBonus, operand) > 0 ? 1 : 0;
    ...
}
```

and `PlayerCharacter.getTotalCheck`:

```java
return getBaseCheck(check) + (int) getTotalBonusTo("SAVE", check.getKeyName());
```

So `PRECHECK` is the total save and `PRECHECKBASE` the class-progression base. The
ability modifier is not a special case in the tester: it arrives as a
`BONUS:SAVE|Fortitude|CON` on the save's own definition in `rsrd__saves.lst`.
`EffectState.total_saves` is therefore `base_saves` plus the save's ability
modifier, built before the effect loop — bonuses applied by *effects* are excluded
for the same reason `total_bonus`/`PRETOTALAB` excludes them: the predicate has to
be answerable while the loop is still running.

**The count was the real defect.** `AbstractPrerequisiteListParser` puts the count
in the operand and the `y=z` pairs in sub-requirements, and the reference's own
example is `PRECHECKBASE:2,Fortitude=5,Reflex=3,Willpower=4` — "any two of the
three". The old lowering took the first part containing `=` and ignored the rest,
so that line became "Fortitude >= 5": a *stricter* test than the reference, and one
that silently rejects characters the feat is meant to allow. Both kinds now lower
to `Predicate::Check { base, specs, min_count }`, with the value taken from the
last `=` segment as §5t requires.

`Predicate::BaseSave` is kept and still evaluates — a pack written before this
change has to keep loading, and `pcgen pack-info` must not fail on it — but nothing
emits it any more. Both the deserialisation and the evaluation are covered by
tests.

### 5y. `PRESPELLDESCRIPTOR`, and where its pairs *add up*

`PRESPELLDESCRIPTOR:x,y=z` looks like `PRESPELLTYPE:x,y=z` and is not: the pairs
behave differently, and the difference is in the tester, not the parser.

`PreSpellDescriptorTester` handles one descriptor at a time:

```java
final String descriptor = prereq.getKey();
final int requiredLevel = Integer.parseInt(prereq.getOperand());
final List<Spell> aArrayList = character.aggregateSpellList("No-Match", "A", descriptor, requiredLevel, 20);
final int runningTotal = prereq.getOperator().compare(aArrayList.size(), 1);
return countedTotal(prereq, runningTotal);
```

`PrerequisiteOperator.compare(a, b)` returns `a` when the comparison passes, and
`countedTotal` hands that straight back for a `countMultiples` sub-requirement —
so the sub-requirement reports the **number of known spells**, not 0/1. The list
wrapper then sums the sub-requirements and compares the total with `x`. Hence
`PRESPELLDESCRIPTOR:4,Mind-Affecting=3` is "at least four 3rd level or higher
Mind-Affecting spells", and `PRESPELLDESCRIPTOR:2,Fire=3,Acid=3` is satisfied by
one Fire spell plus one Acid spell. `PRESPELLTYPE`, by contrast, is described in
the reference as "any of" its types.

The level window is the reference's own `[requiredLevel, 20]`, so a spell the
character knows above 20th level would not count; that is kept rather than
simplified to an open-ended `>=`.

`EffectState.spell_descriptors` maps a lowercased descriptor to the levels of the
draft's known spells that carry it. It has to be precomputed because
`predicate_holds` receives no pack — the same reason `skill_types`,
`ability_traits` and `class_traits` exist.

The reference also lists a deprecated spelling, `PRESPELLDESCRIPTOR:y,x,z`. It is
not supported, and not merely skipped: the current parser rejects it as well,
because `Integer.parseInt(elements[0])` throws on a descriptor name.

### 5z. `PRESPELLSCHOOLSUB` and `PRESPELLSCHOOL`, and the bug next door

`PRESPELLSCHOOLSUB:x,y=z` needed no new reasoning: `PreSpellSchoolSubTester` is
`PreSpellDescriptorTester` with a different slot of `aggregateSpellList` filled
in.

```java
// descriptor tester
character.aggregateSpellList("No-Match", "A", descriptor, requiredLevel, 20);
// sub-school tester
character.aggregateSpellList("No-Match", subSchool, "No-Match", requiredLevel, 20);
```

The `"No-Match"` placeholders are what isolate the slot under test — with a name
that matches no constructed object, only the real slot's `containsInList` can make
the condition true. So the pairs sum exactly as they do for descriptors, and the
level window is again `[requiredLevel, 20]`.

**The divergence next to it, now fixed.** `PRESPELLSCHOOL` *is* implemented, so
it never appears in the unsupported report — but what it did was match the school
name as a substring of the spell's **name**:

```rust
spell.name.to_ascii_lowercase().contains(&school.to_ascii_lowercase())
```

That is not the reference's test. `PreSpellSchoolTester` fills the *school* slot of
`aggregateSpellList`, i.e. it asks whether the spell carries that `SPELL_SCHOOL`.
The substring test can only fire when a spell's **name** happens to contain the
school word, and in the RSRD that is a handful of accidents — `Divination` (the
spell), `Shadow Conjuration`, `Shadow Evocation`, `Break Enchantment` — against
`SCHOOL:` counts of 135 Transmutation, 114 Conjuration, 96 Evocation, 86
Abjuration and so on. `PRESPELLSCHOOL:1,Abjuration=1` can therefore never be
satisfied no matter how many abjurations the character knows.

It is not an edge case either: the third-party data has **504** `PRESPELLSCHOOL`
occurrences, and the acceptance cohort itself uses
`PRESPELLSCHOOL:7,Divination=0`, `PRESPELLSCHOOL:1,Divination=3` and
`PRESPELLSCHOOL:5,Abjuration=5,...`.

The entry also short-circuits on `|| *level == 0`, so an entry whose level is 0 is
vacuously true regardless of the school.

It also dropped the reference's count and joined the entries with `all()`, so
`PRESPELLSCHOOL:5,Abjuration=5,...` meant "all eight schools" rather than "five
spells among them", and it took the value from the first `=` rather than the last.

The fix is the same shape as the sub-school one: `EffectState.spell_schools`
indexes `SpellDefinition.school`, and `PRESPELLSCHOOL` lowers to
`Predicate::SpellSchool { specs, count }`, which sums the per-school counts and
compares with the count. `Predicate::SpellSchoolLevel` is kept so packs written
before this change still load — but it is evaluated with the *corrected* matching,
not the substring test. It never stored a count, so its entries stay "all of them";
preserving the old test would have meant preserving a bug.

**The cohort was unaffected, and that was checked rather than hoped.** The three
`PRESPELLSCHOOL` occurrences in the acceptance data sit on class prerequisite rows
(Archmage, Loremaster). A class entity's requirements are consulted only when the
entity is *granted*; a character that already has levels in a class never re-tests
its prerequisites. So the coverage gate still reports 100% semantic completion and
the broad oracle still agrees with the Java sheets on every exposed field (25/25,
or 23/23 where Java's sheet omits a value). All 504 third-party occurrences, which
sit on feats and abilities, now evaluate properly.

### 5aa. `PRECAMPAIGN`, and the class-level gap it exposed

`PRECAMPAIGN:x,y,y` is documented as a PRE tag for LST objects — "Sets
prerequisites for LST objects based upon the data sets loaded into PCGen. If the
set is loaded it passes, if not it doesn't" — with four selector spellings:
a plain data-set name, `BOOKTYPE=`, `INCLUDES=` and `INCLUDESBOOKTYPE=`.

**It is a question about the pack, not the character.** `PreCampaignTester` reads
`PersistenceManager.getChosenCampaignSourcefiles()` and counts matching campaigns:

* a plain name and `BOOKTYPE=` count only the **selected** campaigns;
* `INCLUDES=` and `INCLUDESBOOKTYPE=` also walk each selected campaign's include
  closure;
* the name is matched against the campaign's **`KEY:`**, because
  `Globals.getCampaignKeyedSilently` compares `getKeyName()` — the display name
  only matters when a campaign declares no key;
* the selectors **sum**, and the total is compared with the count, exactly like the
  spell tags in §5y and §5z.

A compiled pack knows its own `.pcc` graph, and that graph is what a
single-selection session would see, so `CompiledPackHeader.campaigns` records each
campaign's name, key, `BOOKTYPE:` values and whether it is the root (the selected
source) or arrived through a `PCC:` include. The predicate then evaluates against
that list.

**What it exposed.** All nine occurrences in the third-party data are on
*class-level* rows — Complete Divine's 11th-level rows, alongside
`PREPCLEVEL:MIN=20`. And `initial_entities` selects a `CLASS_LEVEL` entity purely
by class and level:

```rust
EntityCategory::ClassLevel => entity.id.key.split_once('#').is_some_and(|(class_id, level)| {
    class_level_is_active(&class_levels, class_id, level.parse::<u32>().unwrap_or(0))
}),
```

No requirement is consulted. `grant_allowed` — the only consumer of an entity's
`requirements` — runs on the *grant* path, so a class-level row's gates are inert.
Lowering `PRECAMPAIGN` therefore preserves the gate and clears the unsupported
report, but does not yet change behaviour; the same is true of `PREPCLEVEL` on the
very same rows, and of every other gate on a class-level row.

That enforcement gap is deliberately left for its own round, and it was measured
rather than guessed before deciding. Two findings matter:

* **The class-level slice is provably a no-op.** Across the 189 third-party packs,
  594 `CLASS_LEVEL` entities carry requirements and **not one of them carries an
  effect**; the 12 in the RSRD cohort are the same (empty level-11 rows gated on
  `PREPCLEVEL:MIN=20`). Enforcing them would change nothing, so it is not worth a
  round on its own.
* **The real gap is identity-selected entities of every other kind.** An entity
  reached by *identity* — a race, a template, an equipment modifier, an explicitly
  chosen ability — never has its own `requirements` tested; only the grant path
  calls `grant_allowed`. In the RSRD cohort the entities that carry both
  requirements and effects are `EQUIPMOD` 402, `TEMPLATE` 155, `ABILITY` 141,
  `EQUIPMENT` 13, `DOMAIN` 6 and `RACE` 4; across the third-party packs the same
  count is in the thousands.

That second item is the one worth doing, and it is a behavioural change large
enough to move cohort numbers, so it needs the full gate and a deliberate decision
rather than a quiet fold-in. It is recorded here so the next reader does not
mistake a lowered gate for an enforced one.

### 5ab. `PRECHARACTERTYPE`, and a default that is not in the pack

`PRECHARACTERTYPE:x,y,y` is the simplest kind of gate — "Sets requisites for
character type" — but it needs a fact the character had never carried.

**The value comes from the game mode, not the pack.** `PreCharactertypeTester`
reads the `CHARACTERTYPE` channel, and `PlayerCharacter` initialises it from
`GameMode.getDefaultCharacterType()`, which returns the first entry of the game
mode's `CHARACTERTYPE:` list.  In 3e and 3.5e that list is declared in
`system/gameModes/{3e,35e}/miscinfo.lst` as `PC|NPC`, so the default is `PC`.  A
game-mode file is outside a pack's `.pcc` graph, so the constant lives in
`pcgen_core::vocab::DEFAULT_CHARACTER_TYPE` with that provenance recorded, rather
than being guessed at the call site.

**The `.pcg` reader was dropping the value.** 18 of the 42 vendored `.pcg` files
declare `CHARACTERTYPE:` — 17 `PC` and one `Monster` — and the tag was not in the
reader's match, so every imported character silently became whatever the default
was.  It is read now, and written back when set.

**A silent file stays silent.** The parser leaves the field empty when the `.pcg`
does not mention it, and the default is applied at the draft boundary
(`draft_from_pcg`).  Filling it in during parsing would have made a write/read
round trip of a `.pcg` that says nothing produce a `CHARACTERTYPE:PC` line, which
is the parser inventing data.

`PreCharactertypeTester` compares the character's type with each listed key and
the list wrapper sums the matches, so `PRECHARACTERTYPE:1,NPC,Monster` is "either"
and a character — who has exactly one type — can only ever match one of them; a
count above one is satisfiable only by a list that names the same type twice.

### 5ac. `PRESA`, and the `SA:`/`SAB:` tags nothing was reading

`PRESA:x,y,y` is a prerequisite about the character's special abilities. The tag
that creates them is `SAB:x`, and the first surprise is that this codebase did not
read it: `SA:`/`SAB:` are in none of the families lowering reports on — not a
`BONUS`, not a `PRE`, not a `CHOOSE` — so **2,378** of them across the 189
third-party packs were dropped without so much as an unsupported count. Modelling
them is therefore the larger half of this round.

**The key name is only the first token.** `SabLst` does

```java
SpecialAbility sa = new SpecialAbility(firstToken.intern());
```

and everything after the first `|` is display text and the variables its `%`
placeholders consume. So `SAB:Sneak Attack +%d%|Sneak Attack|Sneak Attack Die` has
the key name `Sneak Attack +%d%`. The tester then matches with a prefix test:

```java
final String aString = prereq.getKey().toUpperCase();
for (SpecialAbility sa : character.getSpecialAbilityList())
    if (sa.getKeyName().toUpperCase().startsWith(aString)) runningTotal++;
```

which is why `PRESA:1,Sneak Attack` finds it, and why the display text and the `%`
substitution do not have to be modelled at all — they matter to the character
sheet, not to the prerequisite.

**Two qualifiers come with the tag.** "SAB tags can be qualified with PRE tags",
and 21 of the 2,378 occurrences use that, so the prerequisite rides on the effect's
`condition` and a special ability whose gate fails is not granted. `.CLEAR` and
`.CLEAR.Text` (2 occurrences) are list edits *within the same object*, so they are
applied to the effects collected for that record, in tag order.

**Order is a real hazard here.** The prerequisite is evaluated when the effect loop
reaches the entity carrying it, but the special ability may come from a race or a
class row processed later. `special_abilities` is therefore collected from the
initially-active entities *before* the loop, and entities granted during the loop
add theirs as they are applied. Without that, a feat's `PRESA` would depend on the
pack's entity ordering.

The acceptance data has no `SA:`/`SAB:` at all, so this round is provably
cohort-neutral — which is what the coverage gate confirmed rather than assumed.

### 5ad. `PREFACT`, the gate behind most of the `BONUS` queue

`PREFACT` never appeared in the unsupported histogram on its own. It was the gate
*inside* other tags, and that is why the queue looked the way it did: of the 268
`BONUS` occurrences still reported, **245** carried a `PREFACT` condition, and 18
of the 19 `PREMULT` occurrences were blocked by one. Reading the queue by tag name
hid the fact that a single unmodelled prerequisite was holding most of it.

**What the tag asks.** `PreFactTester` walks the objects in an `OutputDB`
location and counts those whose named fact has a given value:

```java
String location = prereq.getCategoryName();
Iterable<Reducible> objModel =
        (Iterable<Reducible>) OutputDB.getIterable(aPC.getCharID(), locationElements);
String[] factinfo = prereq.getKey().split("=");
FactKey<?> fk = FactKey.valueOf(factinfo[0]);
Object targetVal = fk.getFormatManager().convertIndirect(factinfo[1]);
for (Reducible r : objModel)
    if (targetVal.equals(r.getCDOMObject().get(fk))) runningTotal++;
```

and the list wrapper sums the per-fact counts. `PreFactParser` validates the
location against `OutputDB`, whose registry is a `CaseInsensitiveMap` — so
`ABILITIES` and `Abilities` are the same location — and throws when the formula
contains `|`.

**Three decisions worth recording.**

* *Only enumerable locations are lowered.* The location is a facet-backed
  iterable (`abilities` is `GrantedAbilityFacet`, `race` is `RaceFacet`). Those two
  are what the third-party data uses and what the runtime can fill, so any other
  location stays unsupported. Lowering an unknown location would have turned a
  prerequisite into a silent "no" while removing it from the report — the metric
  would improve and the semantics would get worse.
* *A value containing `|` stays unsupported.* `PREFACT:1,RACE,HasFirstLevelFeat=True|PRECLASS:1,ANY=1`
  appears eight times; the reference parser throws on it, so it is malformed data,
  not a form to guess at.
* *Values compare case-insensitively.* Of the 217 fact names the data tests, 215
  are declared `DATAFORMAT:BOOLEAN`, where `convertIndirect` makes `true` and
  `True` the same value — and the data really does mix them
  (`PREFACT:1,Abilities,GriaSurvivialist=True` against `FACT:...|true`). Fact
  *names* need no such care: measured across the packs, no `PREFACT` name differs
  in case from its `FACT` tag.

**What the change does and does not do.** Of the 237 well-formed occurrences, 41
test a fact that some `FACT:` tag actually sets; the other 196 reference facts the
data never sets, so their gates evaluate to **false**. That is a faithful reading
of the data, not a regression — before, those conditions were `Unsupported`, which
also did not pass — and the difference is that the engine now knows the answer
instead of not knowing it. It is worth stating because the count drops by 262
while only 41 of those conditions became genuinely character-dependent.

### 5ae. `PREDR`, where the count is a count and the value has a floor of zero

`PREDR:x,y=z` asks about the character's damage reduction, and the reference is
more surprising than the documentation:

```java
// PreDamageReductionParser
final StringTokenizer inputDRTokenizer = new StringTokenizer(inputTokenizer.nextToken(), "=.");
final String drType = inputDRTokenizer.nextToken(); // either Good.10 or Good=10
...
Prerequisite subprereq = new Prerequisite();
subprereq.setKind("dr");
subprereq.setKey(drType);
subprereq.setOperand(Integer.toString(drValue));
prereq.addPrerequisite(subprereq);
```

Three details decide the model. The pair separator is `=` **or** `.`. A pair with
no value gets `0`. And — unlike the spell and check tags, whose sub-prerequisites
set `countMultiples` — **`PREDR`'s sub-prerequisites do not**, so each contributes
0 or 1 through `countedTotal` and the wrapper really does count how many of the
listed reductions hold. That is what the documentation says ("the number of the DR
conditions that must be met") and, for once, what the code does.

**The value has a floor of zero.** `PrerequisiteOperator.compare` returns 1 rather
than the left side when the left side is 0:

```java
if (passes) {
    if (leftHandOp < 0.0d || CoreUtility.doublesEqual(leftHandOp, 0)) return 1;
    return leftHandOp;
}
```

So a pair with no value — `PREDR:1,Evil`, operand 0 — passes for a character with
*no* Evil reduction at all, because `getDR` returns 0 and `0 >= 0`. The tests pin
that down, because it looks like a bug in the implementation and is not one.

**`getDR` is two halves.** `DamageReductionFacet.getDR` is the object's own
reduction for the type plus `bonusCheckingFacet.getBonus(id, "DR", key)`. In this
codebase `BONUS:DR|Magic|5` lowers to an ordinary modifier on the target
`DR.Magic`, which the engine accumulates in `external_modifiers`, so evaluation
adds that on top of the imported `DR:` rows instead of needing a second pass.

A value with a `|PRE` suffix cannot be read as an integer — the reference's
`Integer.parseInt` fails on it too — so the whole tag stays unsupported rather than
being silently truncated.

### 5af. `PREVAR*`, where four kinds collapsed into one and lost their boundary

`PreVariableParser` takes its operator straight from the kind's suffix:

```java
// Get the comparator type SIZEGTEQ, BSIZE, SIZENEQ etc.
String compType = kind.substring(5);
prereq.setOperator(compType);
```

so `PREVARGTEQ` is `>=`, `PREVARGT` is `>`, `PREVARLTEQ` is `<=` and `PREVARLT` is
`<`. The numeric form of the lowering always kept that: it produces
`Predicate::Variable { min, max }` and shifts the strict kinds by one, which is
exact because `EffectState::variables` is an integer map (`x > 5` is `x >= 6`;
PCGen's own variables are floats, where it would not be).

The **symbolic** form did not. All four kinds lowered to the same shape:

```rust
Predicate::VariableCompare {
    left, right,
    greater: matches!(tag, "PREVARGT" | "PREVARGTEQ"),
}
```

and the engine evaluated `greater` as `>` and everything else as `<`. `GTEQ` and
`LTEQ` were therefore compared one step too strictly. It is a quiet kind of wrong:
the predicate still lowers, nothing is reported, and the effect simply appears one
point later than it should.

**How much of the data is affected** was measured rather than guessed. Of the
34,742 `PREVAR*` pairs in the third-party packs, 2,519 have a symbolic operand —
the only path that lost the boundary: `PREVARGTEQ` 181 of 31,385, `PREVARLTEQ` 132
of 3,352, `PREVARGT` 31 of 350, and `PREVARLT` 2,175 of 2,782. The RSRD has no
`PREVAR*` at all, so the acceptance cohort cannot be affected.

**A second defect sat next to it.** `PreVariableTester` reads both sides through
`getVariableValue`, which evaluates its argument, so the left operand of a symbolic
comparison is often a plain number: `PREVARGTEQ:3,EmpowerHealingLVL` is
"3 >= EmpowerHealingLVL". `variable_value` looked the text up as a variable name
and fell back to 0, which turned those comparisons upside down. It now falls back
to parsing an integer. A genuine formula operand such as
`ceil((CL=Horizon Walker)/10)` still resolves to 0 — a gap this round does not
close, recorded here so the next reader does not mistake the literal fallback for
full expression evaluation.

Old packs keep loading: `strict` is `serde(default)`, so a pack written before the
boundary was kept reads as the inclusive form — which is what `GTEQ` and `LTEQ`
mean, and 34,737 of the 34,742 pairs use those two kinds.

### 5ag. `PREREACH`, whose operator lives in the tag name

`PreReachParser` handles seven kinds and takes the comparison straight out of the
kind's suffix:

```java
String compType = kind.substring(5);
if (compType.isEmpty()) compType = "gteq";
prereq.setOperator(compType);
prereq.setOperand(formula);
```

so `PREREACH:10` and `PREREACHGTEQ:10` are the same test, `PREREACHLTEQ` is `<=`,
and `PREREACHGT`/`LT`/`EQ`/`NEQ` fill out the set. `PreReachTester` then compares
`ReachFacet.getReach` with the operand and returns 0 or 1.

The two directions are modelled as `ReachAtLeast` / `ReachAtMost` and the other
four kinds are **composed** from them — `GT` is "at least and not at most", `LT` the
reverse, `EQ` both, `NEQ` the negation — which is how `PRESIZEEQ` and `PRESIZELT`
already compose the size family, and avoids inventing a comparison-operator type for
one family.

**Where the reach comes from** is the part worth writing down, because it is
narrower than it looks:

```java
final Race aRace = raceFacet.get(id);
int reach = 0;
if (aRace != null) reach = aRace.getSafe(IntegerKey.REACH);
for (PCTemplate template : templateFacet.getSet(id)) {
    Integer r = template.get(IntegerKey.REACH);
    if (r != null) reach = r;
}
reach += (int) bonusCheckingFacet.getBonus(id, "COMBAT", "REACH");
```

Three things follow. `IntegerKey.REACH` is declared as `getConstant("REACH", 5)`, so
`getSafe` returns **5**, not 0, when the race never sets the key. A template
*overrides* the race rather than adding to it — and the last template with a value
wins, not the largest. And `BONUS:COMBAT|REACH` is added last, which in this
codebase is an ordinary modifier on the target `COMBAT.REACH` in
`external_modifiers`, so evaluation adds it without needing a second pass.

**Abilities are deliberately not consulted.** In the third-party data most `REACH:`
rows sit on wild-shape forms, which looks like an ability carrying reach — but they
are in `ph_templates.lst`, and `ReachFacet` reads only the race and the templates.
Reading every active entity's reach (which is what the snapshot's merged
`reach_feet` does) would have made `PREREACHGTEQ:10` true for a druid who had merely
*learned* a form.

The operand must be an integer: `Integer.parseInt` is what the tester calls, so
`PREREACHGTEQ:CL+5` stays unsupported rather than being evaluated as a formula.

### 5ah. `PREITEM`, which was asking about the wrong verb

`PREITEM:x,y,y` reads as "the number of items a character must **possess**", and the
test says the same:

```java
if (display.hasEquipment())
    for (Equipment eq : display.getEquipmentSet())
```

— the possessed set, not the equipped one. The lowering that existed before this
round produced `Predicate::Equipped`, which answers a different question, and it
understood only `TYPE=` entries, so a name entry such as
`PREITEM:1,Dukar Hand Coral` was reported unsupported while
`PREITEM:1,TYPE=Natural` quietly tested the wrong thing. Both halves are fixed
here: `Predicate::Item` carries name and type entries, and evaluation walks
`draft.equipment`.

**The two entry kinds count differently**, and the difference is in the source:

```java
if (aString.indexOf('%') >= 0) {
    final int percentPos = aString.indexOf('%');
    final String substring = aString.substring(0, percentPos).toUpperCase();
    if (eqName.startsWith(substring)) { ++runningTotal; break; }
}
else if (eqName.equalsIgnoreCase(aString)) { ++runningTotal; break; }
```

The `break` means a *name* entry contributes at most one no matter how many items
match it — and the wildcard is a prefix test, with everything from the first `%` on
ignored, so `%` alone matches the first item of any name. The `TYPE` branch has no
`break`, so it contributes every matching item, and it requires *all* the
dot-separated types:

```java
String stripped = aString.substring(Constants.SUBSTRING_LENGTH_FIVE);
typeList = CoreUtility.split(stripped, '.');
...
for (String s : typeList) if (!eq.isType(s)) { bMatches = false; break; }
```

The wrapper then sums the entries, which is why `PREITEM:2,TYPE=Armor,TYPE=Armor` is
satisfied by a single suit of armour.

**Types come from the catalogue, through the same lookup the rest of the engine
uses.** A possessed name is resolved to an `EquipmentDefinition` by display name,
then by a customised item's `BASEITEM`, then as written — the order
`Engine::resolve_equipment` already applies — so `+2 scale mail` still counts as
armour for `TYPE=Armor`. Without that step a customised item would have no types at
all, which is the sort of gap that looks like a data problem and is not one.

### 5ai. `PRECLASSLEVELMAX`, which is `PRECLASS` written backwards

There is no `PreClassLevelMaxTester`. The parser rewrites the tag into the inverse of
`PRECLASS` and documents the transformation in its own comment:

```text
// ClassLevelMax is basically the inverse of class
//
// PRECLASSLEVELMAX:Barbarian=1     !PRECLASS:Barbarian=2
// !PRECLASSLEVELMAX:Barbarian=1    PRECLASS:Barbarian=2
// PRECLASSLEVELMAX:Fighter=1,SPELLCASTER=2  PREMULT:2,[!PRECLASS:Fighter=2],[!PRECLASS:SPELLCASTER=3]
```

and does it by hand:

```java
oper = Integer.toString(Integer.parseInt(oper) + 1);
p.setOperand(oper);
p.setOperator(PrerequisiteOperator.LT);
p.setCountMultiples(false);
```

`class_levels < z + 1` is `class_levels <= z`, so `y=z` means "at most z levels", and
clearing `countMultiples` is what makes each entry contribute 0 or 1 to the group's
count rather than the class levels themselves. Modelling it as
`Not(ClassRequirement { spec, min_level: z + 1 })` inside an `AtLeast { n: x }` needs
no new tester and no engine change, and the reference's own example is a test case.

A pair with no level (`PRECLASSLEVELMAX:1,Fighter`) is deliberately left unsupported.
The parser's comment says such an entry becomes `!PRECLASS:Fighter=3` — the *count*
plus one — while the generic list parser it delegates to assigns a valueless element
the operand `1`. The two readings disagree and the third-party data always states a
level, so guessing would be worse than staying visible.

#### A divergence found while verifying this one, not fixed here

`PreClassTester` aggregates a `TYPE=X` entry differently depending on whether the
sub-prerequisite counts multiples:

```java
if (cl.isType(typeString)) {
    if (prereq.isCountMultiples()) {
        if (display.getLevel(cl) >= preClass) countedTotal++;
    } else {
        runningTotal = Math.max(runningTotal, display.getLevel(cl));
    }
}
```

With `countMultiples` — which the list parser sets when the tag lists more than one
entry — it counts the classes of that type that reach the level. Without it, which is
the single-entry form `PRECLASS:1,TYPE.Base=2`, it takes the **maximum** level among
them. `class_spec_holds` currently **sums** the levels of every matching class
instead, so a Fighter 1 / Wizard 1 fails `PRECLASS:1,TYPE.Base=2` here and passes it
in PCGen.

That is a pre-existing divergence in `PRECLASS`, not something this round introduces,
and it reaches the acceptance data: the RSRD has 51 `TYPE.` values. It is recorded
rather than fixed here because it changes cohort numbers and the current batch is
still unverified — a cohort-moving change should land on a green baseline, so that a
gate failure can be attributed to it rather than to the batch. It is the next
candidate once the batch verification is done.

### 5aj. `PRESKILLTOT`, where the parser's shortcuts become the semantics

`PRESKILLTOT:Spot,Listen,Search=30` reads like a variant of `PRESKILL`, and the two
do share a vocabulary, but the aggregation is the opposite. `PreSkillTotalParser`
marks every listed skill `totalValues`:

```java
Prerequisite subreq = new Prerequisite();
subreq.setKind("skill");
subreq.setKey(token);
subreq.setTotalValues(true);
prereq.addPrerequisite(subreq);
```

and `PreSkillTester` reads that flag in two places:

```java
if (prereq.isTotalValues()) {
    runningTotal += SkillRankControl.getTotalRank(character, aSkill).intValue();
}
...
if (prereq.isCountMultiples() || prereq.isTotalValues()) {
    // For counted totals we want to count all occurances, not just the first
    foundMatch = false;
}
```

So it *adds* ranks instead of counting skills that clear a threshold, and it keeps
scanning after a match — which is what makes a `TYPE.Text` entry sum every skill of
that type rather than asking whether one of them is high enough. The group then
compares the total with the operand, and the operand is whatever the last token
carrying an `=` set. That last detail is not a detail: a formula with no `=` at all
(`PRESKILLTOT:Spot,Listen`) leaves the operand at 0 and is therefore **always
satisfied**, and the code is kept rather than tidied into "the total is required".

**Two forms are rejected by the reference itself**, and both stay unsupported here
rather than being guessed at. The value is read with `Integer.parseInt` off the
*first* `=`, so `Spot=Listen` throws; and that same first-`=` split makes the
`TYPE=Text` spelling unusable for this tag, because `TYPE=Knowledge=8` hands
`Integer.parseInt` the string `Knowledge`. Only `TYPE.Text=8` works, which is the
spelling the data and the tests use.

**The third-party form is degenerate but real.** All three occurrences are
`PRESKILLTOT:=1` — one entry whose skill name is empty. The reference accepts it, no
skill has an empty name, so the total is 0 and the requirement can never hold.
Lowering it is the faithful answer: the tag is now *known* to be false instead of
unknown. The test pins that down, because it looks like a modelling gap and is not
one.

**One bug only a sum could expose.** `EffectState.skill_types` records both a
skill's id and its name for each of its type tags, so a skill whose key and name
agree appears twice. A count of matching skills is unaffected; a sum would double
them, so `skill_total_ranks` de-duplicates by lower-cased name before adding.

### 5ak. `PREUATT`, which is not an unarmed attack bonus

The documentation calls `PREUATT` "the minimum unarmed attack bonus", the parser
handles the kind `UATT`, and the tester is `PreUnarmedAttackTester`. All three say
the same thing, and all three are wrong about what the code does:

```java
int att = 0;
for (PCClass aClass : display.getClassSet())
{
    att = Math.max(att, aClass.baseAttackBonus(character));
}
final int requiredValue = Integer.parseInt(prereq.getOperand());
final int runningTotal = prereq.getOperator().compare(att, requiredValue);
```

There is no unarmed-attack statistic anywhere in it. It is the **maximum base
attack bonus any single class contributes** — not the character's total, which is
what `PRETOTALAB` gives, and not an unarmed-specific value.

The difference is not academic for a multiclass character. A Fighter 4 / Wizard 4
has a good progression worth 4 from the fighter levels and a poor one worth 2 from
the wizard levels; their total is 6, which is what `PRETOTALAB` reports, while
`PREUATT` sees 4. A test pins both numbers on the same character, because the
distinction is invisible with a single class and that is the case the tag's
description invites you to assume.

The variant is therefore called `HighestClassBab` rather than something containing
`Unarmed`: a reader looking for the tag will find it through the lowering arm and
the doc comment, and a reader looking at the enum will not be misled by a name that
the reference itself gets wrong. The computation lives in `bab.rs` beside
`derive_prereq_bab`, so the two base-attack-bonus notions stay side by side.

One extension: `effective_class_levels` also covers advancement lanes, so a gestalt
draft takes the maximum over its lanes. PCGen has no gestalt, so there is no
reference behaviour to match — it is the closest analogue, and it is recorded here
rather than left to be inferred.

### 5al. `PRESPELLCAST`, where the spell type was typed and only one token was read

This tag had two defects, and both came from treating a list of text tokens as if
it were one typed value.

**Only the first token was read.** `PreSpellCastParser` splits the formula on commas
and builds one sub-requirement per token:

```java
if (token.startsWith("MEMORIZE")) { ... setKind("spellcast.memorize") ... }
else if (token.startsWith("TYPE")) { ... setKind("spellcast.type"); subprereq.setKey(token.substring(5)); }
else throw new PersistenceLayerException(...);
```

and the group keeps the parser's default operand of 1, so two tokens mean "at least
one of them holds"; the wrapper is dropped only when there is a single
sub-requirement. The old lowering searched for the first `MEMORIZE=` or the first
`TYPE=` and ignored the rest, so `PRESPELLCAST:TYPE=Arcane,TYPE=Divine` — nine
occurrences in the third-party data — asked about Arcane alone and failed for a
cleric.

**The spell type was typed, and it is text.**

```java
final String prereqSpellType = prereq.getKey();
for (PCClass aClass : display.getClassSet())
    if (prereqSpellType.equalsIgnoreCase(aClass.getSpellType())) runningTotal++;
```

and `getSpellType` is the class's `FACT:SpellType` verbatim, or `None` when the fact
is unset:

```java
FactKey<String> fk = FactKey.valueOf("SpellType");
String castInfo = getResolved(fk);
return StringUtils.isEmpty(castInfo) ? Constants.NONE : castInfo;
```

So `PRESPELLCAST:TYPE=Arcane=1` — which the data contains twice — asks for a class
whose spell type is literally `Arcane=1`. No class has one, so the answer is
**false**. The old lowering ran the value through `SpellPowerType::from_pcgen`,
which knows `arcane`, `divine` and `psionic` and nothing else, so the whole tag
became unreadable — and took the `PREMULT` wrapping it down as well. That is why
this one fix closed the last two entries in the `PREMULT` column.

`HasSpellPower` is gone with it, replaced by `HasSpellType { spell_type: String }`,
and the character's classes' strings are collected into `EffectState.spell_types`
from `effective_class_levels` rather than from `class_traits` — the latter holds
every class in the pack, and a prerequisite about *the character's* classes must not
be answered by a pack's catalogue.

The acceptance cohort contains no `PRESPELLCAST` at all, so the change is provably
cohort-neutral, and the coverage gate confirmed it rather than assuming it.

### 5am. `PRESPELLTYPE`, which had the same typing problem as `PRESPELLCAST`

The round before this one made `PRESPELLCAST`'s spell type a string.  This one does
the same for `PRESPELLTYPE`, and for the same reason: the reference treats the type
as text everywhere, and the data has types the three-variant enum cannot name.
Eighteen of the twenty-three entries still in the third-party queue were
`PRESPELLTYPE:1,Shadow Magic=N` — `Shadow Magic` is a real spell type in that data,
and the typed field is what made the tag unreadable.

`PreSpellTypeTester` does very little of its own work:

```java
final String castingType = prereq.getKey();
try { requiredLevel = Integer.parseInt(prereq.getOperand()); }
catch (NumberFormatException e) { requiredLevel = 1; ... }
int count = character.countSpellCastTypeLevel(castingType, requiredLevel);
final int runningTotal = prereq.getOperator().compare(count, 1);
```

and `countSpellCastTypeLevel` matches the class's `FACT:SpellType` with
`equalsIgnoreCase`, treating `"Any"` as the wildcard — and **skipping** a class whose
fact is unset, because it reads the raw fact rather than going through
`getSpellType`.  That last point is why `ClassDefinition.spell_type` keeps the fact's
raw text and stays *empty* when the class declares none: `PRESPELLCAST` needs `None`
there and `PRESPELLTYPE` needs "no entry at all", and the two cannot both be
expressed by baking one of them into the parsed value.

Three parser details are modelled rather than approximated, all of them read out of
`AbstractPrerequisiteListParser` rather than guessed:

* the pair is split on its **last** `=`; the reference takes
  `tokens[tokens.length - 1]` as the value and rebuilds the key from the rest, so
  `Shadow Magic=1` is the type `Shadow Magic` at level 1;
* `=ANY` is a legal value (`isAnyLegal()` defaults to true) and the tester's
  `NumberFormatException` handler turns it into level 1 — which is why the level is
  no longer simply "must parse";
* an element with no `=` at all, and a level that is neither a number nor `ANY`,
  both make the parser throw, so they stay unsupported instead of being guessed at.

Each pair is counted on its own and the counts are summed before the comparison,
because the list wrapper marks its sub-requirements `countMultiples`; two pairs
naming the same type therefore count its slots twice, exactly as the reference does.

The stored state changes with it: `EffectState.slots_by_power_level` becomes
`spell_slots_by_type`, keyed by the spell type text.  That is also closer to
`countSpellCastTypeLevel`, which has no spellcasting-ability filter — a class with a
custom spell type and a `CAST` table answers this prerequisite in PCGen whether or
not it declares a `SPELLSTAT`.

`SpellTypeSpec` is serialised in every stored pack, so the field is renamed with
`#[serde(default, alias = "power")]`: a pack written while it was a `SpellPowerType`
carries `"power": "arcane"`, which reads back as the text `arcane` — the same
comparison, case-insensitively — and `"power": null`, which meant `ANY`, still means
`ANY`.  A test loads both shapes.

### 5an. The `PRELEGS` family, and the constant behind it

The queue entry was one tag — `PRELEGSGT:2` — but reading the family showed that the
missing spelling was the least of it.  `EffectState.legs` was hard-coded:

```rust
/// Number of legs, when known; defaults to biped (2).
pub legs: u32,
...
legs: 2,
```

and nothing anywhere parsed the `LEGS:` tag, which the data puts on race rows
(`LEGS:2`, `LEGS:4`, `LEGS:0`, and up to `LEGS:120`).  So the 399 `PRELEGS*`
occurrences in the third-party data were not predicates at all: 233
`PRELEGSGTEQ:4` were always false and 162 `PRELEGSLTEQ:3` always true, for every
creature, including the four-legged ones they were written about.

`LegsFacet.getLegs` is short and completely specified:

```java
Race aRace = raceFacet.get(id);
int legs = 0;
if (aRace != null) legs = aRace.getSafe(IntegerKey.LEGS);   // default 2
for (PCTemplate template : templateFacet.getSet(id))
{
    Integer l = template.get(IntegerKey.LEGS);
    if (l != null) legs = l;                                 // last one wins
}
```

so the race supplies the number — 2 when it states none, and **nothing** when there
is no race at all — and each template that states one replaces it.  The template
facet is an `AbstractSourcedListFacet`, so that "last one wins" runs over a real
order: the race's own templates first, then the applied ones, which is the order the
engine walks here.

On top of the value, `PreLegsParser` turns the kind's suffix into the operator
directly (`LEGSGTEQ` -> `GTEQ`, and the bare `LEGS` -> `gteq`), and `PreLegsTester`
reads the operand with `Integer.parseInt`, so a formula is an exception there rather
than a number.

The six non-`gteq` spellings deliberately add **no IR variants**.  With `at_least`
being `LegsAtLeast` and `at_most` being `LegsAtMost`, the other four operators are

| spelling | predicate |
| --- | --- |
| `PRELEGSGT` | `Not(LegsAtMost)` |
| `PRELEGSLT` | `Not(LegsAtLeast)` |
| `PRELEGSEQ` | `All[LegsAtLeast, LegsAtMost]` |
| `PRELEGSNEQ` | `Not(All[LegsAtLeast, LegsAtMost])` |

which is exactly the same comparison written the other way round.  A count of legs
is an integer, so the negations are exact rather than approximate, and the IR does
not grow four variants that would each restate one of these.

`LEGS` is also added to `consumed_legacy_tag`, beside `REACH`, which
`parse_creature_traits` reads the same way — a tag that is parsed should not also be
reported as pending.

### 5ao. The `PREEQUIP*` keys, which were never read as keys

This one was not in the queue.  The queue had two `PREEQUIPTWOWEAPON` rows, and
reading the family around them showed that every key of every member was being kept
as raw lower-cased text:

```rust
let values: Vec<String> = parts.iter().skip(1)...map(|part| part.to_ascii_lowercase()).collect();
...
let matches = |item: &String, value: &str| item == value || item.contains(value);
```

So `PREEQUIP:1,TYPE=Weapon` asked whether any item's name contains `type=weapon`.
Nothing is named that, so it was **silently false** — for 450 of the 575 `PREEQUIP*`
occurrences in the third-party data, and 208 more in the Pathfinder set.  (The
acceptance packs contain none, which is why no gate ever noticed.)

Both testers behind the family branch on the key's prefix, and the branches are what
`ItemSpec` now keeps:

```java
if (targetEquip.startsWith("WIELDCATEGORY=") || targetEquip.startsWith("WIELDCATEGORY."))
    ... wCat.getKeyName().equalsIgnoreCase(targetEquip.substring(14)) ...
else if (targetEquip.startsWith("TYPE=") || targetEquip.startsWith("TYPE."))
    // StringTokenizer(targetEquip.substring(5), ".") and *every* token must match
else { ... BASEITEM= reads getBaseItemName(), otherwise getName(), with a `%` wildcard ... }
```

`ItemSpec` gained `BaseItem(String)` and `WieldCategory(String)` beside `Name` and
`Types`, and `Equipped` gained the `specs` list that holds them.  Two details are
worth naming.  The wield comparison folds case, spaces, underscores and hyphens,
because the data spells a category `TwoHanded` while the reference's key names and
the `.lst` files disagree about separators; and the `%` wildcard lives in the pattern
rather than being expanded, so `Longsword%` and `Longsword` differ in exactly the way
the reference's `regionMatches` and `equalsIgnoreCase` do.

The other thing the old lowering dropped was the **count**.  The list wrapper counts
one per key — each key's tester answers `0` or `1`, never more — so
`PREEQUIP:2,Chain,Sword` wants both keys to match while `PREEQUIP:2,Chain` can never
be satisfied, and `PREEQUIP:0,...` always is.  `ItemSpec`'s list therefore carries a
`min_count`, defaulting to 1 through `serde` so that packs written before it existed
keep the meaning they had (any key matching).

Checking this in exposed one more thing, which the batch round fixed: the hand slots
were a **union** rather than a partition — `PREEQUIPPRIMARY` searched
`primary | both` and `PREEQUIPSECONDARY` searched `secondary | both`, while the
reference's `EquipmentLocation` holds exactly one value per item and a two-handed
weapon is `EQUIPPED_BOTH`, not a primary one.  With name keys the difference was
invisible, because a name only ever matched its own item; the moment `TYPE=` keys
started matching, a greatsword answered `PREEQUIPPRIMARY`.  Both paths now search
the single slot they name.

The `BONUS` qualifier path (`equipped_condition`) still builds the loose `values`
form, deliberately: those qualifiers were written to be passed through as text, and
typing them here would change what existing characters' bonuses mean.  The engine
reads `specs` when it is non-empty and falls back to `values` otherwise, so both
shapes work and only the tag path moved.

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
