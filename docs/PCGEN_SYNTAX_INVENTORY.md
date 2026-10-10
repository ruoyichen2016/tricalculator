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

The remaining kinds are genuinely unimplemented: `PRE` (21), `PRESA` (12),
`PRESKILLTOT`, `PREHP`, `PRESIZENEQ`,
`PREDEITYDOMAIN`, `PRESRLT`,
`PRECLASSLEVELMAX`, `PRETEXT`. None appear in the
acceptance cohort.  Several names on the original list have since been lowered:
`PRELEVELMAX`, `PREATT`, `PREHANDSGT`, multi-mode `PREMOVE`, `PREREGION`
(see §5u), `PRECSKILL` (see §5v), `PREALIGN`'s value forms (see §5w),
`PRECHECK` (see §5x), `PRESPELLDESCRIPTOR` (see §5y), `PRESPELLSCHOOLSUB` and
`PRESPELLSCHOOL` (see §5z); `PRELEVEL` now supports both `MIN=` and `MAX=`.  See
the current `CHANGELOG.md`.

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
