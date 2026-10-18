# Gladius architecture

Gladius is a rolling D&D 3.5 rules workbench. Local raw and accompanying rule prose are the semantic authority. Imported machine tags are evidence, not an oracle when they contradict the feature text. There is no native-save migration, backup branch, default full test suite or full rules gate.

The central question is: **what acquired rule allows this character to perform this operation at this time?** Numerical availability, chronological qualification and encounter execution are separate answers.

## Boundaries

```text
RAW files + source locations + conflict decisions
                    ↓ import / native rules
             CompiledRulePack
                    │
Class frames + acquisition history ──→ historical projections ──→ qualification audit
                    │
                    └──→ current projection ──→ CharacterSnapshot / spell catalog
                                      │
Current preparations + encounter state + operation request
                                      ↓
                           operation plan / preview
                                      ↓
                     revalidate + atomic state transition
                                      ↓
                      ordinary forms / reports / CLI
```

| Responsibility | Implementation | Boundary |
|---|---|---|
| Source ingestion | `pcgen-import`, `pcgen-ir`, `native.rs` | Read files, lower vocabulary, retain source provenance. No runtime source-text interpretation. |
| Native rules | `data/rules/*.rules.json`, `CompiledRulePack` | Tables, predicates, casting policies, features and explicit pending mechanics. No character state. |
| Acquisition inputs | `core::advancement`, `core::build`, `CharacterDraft` | Ordered class frames and skill/feat/spell/choice/evidence events. No stored derived bonuses. |
| Projection and qualification | `core::build`, `engine::build`, `engine::progression`, `engine::effects` | Reconstruct the relevant time and evaluate shared rule predicates. Never infer historical acquisitions from final totals. |
| Operations | `engine::execution`, `magic`, `class_resources`, combat/effect resolvers | Read current resources, preview, revalidate, return a new state. No browser-owned resource balances. |
| Storage and presentation | `pcgen-builder`, `pcgen-web`, CLI/output | Persist inputs and render engine results. No second UI rules engine. |

Existing crates are sufficient. Formula evaluation remains in `pcgen-formula`; the engine depends on domain types rather than importer/IR internals. `web::rules::RuleStore` caches rule engines. Native changes retain source-qualified identities and referenced raw locations.

## One class history, explicit acquisition order

`CharacterDraft.advancement` is the only writable class-level history. Each `AdvancementFrame` is a character level and contains one or more lanes; `class_levels()` is a derived summary. Actual class level, advanced casting level, caster level and initiator level are different quantities.

`build_history` records original scores/alignment, a declared `recorded_through` level, and ordered `BuildEvent` inputs:

- skill rank increments;
- feat acquisition and explicit feat retraining;
- learned spells bound to a casting track;
- class choices and prerequisite evidence;
- level-up ability increases.

Class entry is checked against the end of the previous character level. Feats are checked after the current class level and only the acquisitions preceding that feat. Moving an event within a level can therefore change its qualification result. Retraining does not rewrite the earlier event; unresolved original-acquisition requirements and dependencies produce a ruling requirement.

`build_at(level, event_limit)` strips final skills, feats, choices, spells, current equipment and encounter state before reconstructing the requested point. Current preparations are excluded. `materialized()` produces the current projection and preserves current equipment, preparations and encounter resources. Projections carry no history, preventing recursive evaluation. Repository loads and rolling replacements refresh the projection; engine evaluation and casting also project explicitly at their boundaries. Legacy flat selection fields are only a current projection when a history is present, never evidence of earlier qualification.

Missing history is **Missing Evidence**, not an invented acquisition schedule. PCG import creates no history. The Build form starts an explicit history only from user-entered original scores and existing class frames; it warns that final selections are replaced by recorded acquisitions. Structurally invalid events cannot be projected, while reconstructible but rule-invalid builds can be saved for correction.

The ordinary Build form edits single-lane class frames. Existing gestalt calculations remain in the engine; the new form refuses to silently flatten a multi-lane build. Skill-point budgets and gestalt progression remain centralized in `engine::progression`; the acquisition audit currently checks rank caps but does not certify complete skill spending.

## Decisions and evidence

`RuleDecision` carries a level, subject, status, explanation and source. The common statuses are:

| Status | Meaning |
|---|---|
| Satisfied | This particular check has supporting inputs. |
| Failed | Recorded inputs violate the checked rule. |
| Missing Evidence | Required history or external evidence is absent. |
| Needs Ruling | Interpretation or a retraining dependency needs explicit adjudication. |
| Not Implemented | The relevant automatic check or resolver is unavailable. |

The audit never equates an available snapshot with a legal build. Incomplete source-specific feat allocation, complete skill budgets, known-spell quotas and full retraining dependencies remain explicit coverage limits. Requirement lookup distinguishes real feats from same-named class bonus abilities; display names alone are insufficient identity.

The three extreme cases use `audit_sequence` for acquisition checks. Their adapters retain supplied input, claims and bounded encounter assumptions. They no longer maintain separate BAB feat gates, early-entry rank arithmetic, alignment gates or Divine Metamagic acquisition timing. Source-specific faith notes and supported encounter effect models remain separate from the generic acquisition engine.

## Spell access, knowledge, preparation and execution

A casting track owns its class power, ability, slot/known progression and advancement targets. Learning arcane spells cannot retype a divine class. `ListAccess` is scoped where necessary; adding Favored Soul cannot lend its Cleric list to another track.

`ClassMagic.save_dc_ability` is shared by the snapshot and cast preview. Favored Soul uses CHA for casting/slots and WIS for DCs. Base spell level, effective spell level, required/paid slot and caster level remain distinct outputs.

Learned spells are acquisitions. `CharacterDraft.preparations` is current-day input and cannot grant knowledge or satisfy historical prerequisites. `prepared_copies()` is the shared preparation lookup. The ordinary preparation form checks accessible known spells, casting ability, ordinary slot capacity and prior expenditure. Restricted specialist/domain slots and prepared metamagic variants are not silently assigned by this form.

`plan_cast` funds the request from saved counters and calls the existing cast resolver. Its trace separates access, transformations, payment, action timing, targets and rulings. `execute_cast` rejects stale revisions and calls the existing immutable-input/new-state-output transition. Ordinary casting and quiescent storage revalidate before returning a changed state; failure does not persist partial costs. Domain turning, feat/class resources, spell slots and daily preparations retain separate ledgers.

`core::actions::TurnBudget` is shared by the martial and incarnum replays. It spends standard/move/swift/full-round actions, permits a standard action to substitute for a move, and accepts explicit extra-action grants. Failed spending leaves the budget unchanged. Replay-local budgets are not presented as a complete live encounter scheduler. Spell plans expose typed costs when known; immediate/longer or unknown timing is explicitly unsupported by this budget. Live casting records resource use and effects, but does not automatically arbitrate all combatants' turn expenditure.

Spellguard requires a verified defensive personal arcane spell profile; an extraordinary temporary-HP rider does not change spell type. `spell_features` remains independent of the Joker fixture. Target geometry, defenses, touch charges, afflictions and tactical effects remain in their existing specialized resolvers, called through shared runtime state rather than duplicated in the UI.

## GUI

All pages are English server-rendered HTML with ordinary GET/POST forms. No JavaScript, background polling, client-side rules or duplicate resource calculations.

| Navigation | Route | Purpose |
|---|---|---|
| Character | `/` | Current values, acquisition status, blockers and links. |
| Build | `/build` | Per-level class and ordered acquisition editing; original scores; qualified-at-acquisition evidence. |
| Spellcasting | `/spellcasting` | Select a casting track and known spell, record preparations, preview or commit an operation. |
| Encounter | `/class-play` | Existing target, resource, effect, damage, defense and tactical forms. |
| Rules | `/classes` | Complete level tables, source evidence and implementation coverage. |
| Cases | `/cases` | Original claims, shared acquisition audit and links to isolated experiments. |

`/editor` creates/configures characters without an acquisition history. Once history is active, it directs editing to Build rather than overwriting derived final choices. `/workbench` retains advanced casting proofs and specialized previews; it is not the primary navigation. `/martial`, `/incarnum` and `/joker` remain isolated experiment forms reached from Cases. A successful experiment does not repair the original case or save character resources.

New mutating forms use a shared mutation lock, current revision checks and rolling replacement without history backup files. Controls show acquired metamagic options; advanced exceptional proofs remain in the advanced workbench. Starting inputs and events are ordinary fields, not JSON editors. The shared page shell, stylesheet, escaping and semantic tables/details remain in `web::ui` and `web::forms`.

## Implementation coverage

`FeatureLevel.coverage` separates **data**, **qualification** and **execution**. A recorded twenty-level table is not a claim that every ability is executable. Unassessed features are visibly Not Assessed; known partial/unimplemented Joker features carry explicit notes. Native pending mechanics also appear in acquisition reports instead of being silently omitted.

See `docs/SHADOW_POUNCE_AUDIT.md`, `docs/WARFORGED_UMBRAL_AUDIT.md` and `docs/JOKER_AUDIT.md` for source decisions and bounded feature execution. The fixture `.audit.json` files are generated reports, not rules or expected numerical oracles.

## Validation and delivery

Use the smallest relevant checks: `build_history`, the three existing case tests, and the selected Rainbow/War Weaver regression when shared casting changes. HTTP workflow checks cover history creation/reordering, invalid-input rollback, current-resource previews, successful casting, stale submissions and the rule/case pages. Build the Web executable, update `dist/bin/pcgen-web` by rolling replacement and run `cargo clean --profile dev`. Do not run the full gate by default.
