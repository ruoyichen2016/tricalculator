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
| `classes.*.prohibited_spells` | 16 | no | no | carried only |
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

### 5. Note: `GrantTag` carries conditions it does not evaluate

`WEAPONPROF|TYPE=Martial|PREHD:MAX=0` is split on `|` and every part is inserted
as a weapon proficiency name, so `TYPE=Martial` and `PREHD:MAX=0` become literal
proficiencies. The engine does not interpret them, but it also does not strip
them. Latent, 2 occurrences, recorded here rather than acted on.

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
