# Shadow Pounce / Ruby Knight Vindicator audit

The original forum build is **not legal under the unadapted rules**. A separate,
explicitly counterfactual resource replay can produce 13 full attacks from its
claimed 29 turning attempts. That replay is not a legal-build certificate.

## Class coverage before combination

The focused probe traverses Cleric 1–20, Crusader 1–20, Swordsage 1–20,
Swashbuckler 1–20, Crinti Shadow Marauder 1–5 and Ruby Knight Vindicator 1–10:
95 class levels before the multiclass case. Cleric uses the existing RSRD pack.
The five added classes have complete BAB/save/skill progressions and feature
acquisition records. Swordsage/Crusader known, readied and stance tables are
class-level tables; RKV awards are assigned to a selected existing track.
RKV divine casting advances at 2–5 and 7–10; it does not advance turning level.
Crusader Mettle is acquired at 13: the local ability description and published
table agree; the local class grant at 12 is erroneous.

The 208 local maneuver records are a separate typed catalog, not spells.
Acquisition validation checks the acquisition frame, source class, receiving
track, maximum maneuver level, discipline prerequisites, first-level stance,
exchanges and per-level awards. Initiator level is calculated separately for
each base martial class, with full martial prestige levels and half other levels.

**Execution boundary:** this change does not implement every effect of all 208
maneuvers or every class ability's encounter resolution. Full-level descriptions
and numeric tiers are distinct from executable abilities. Those remaining
class encounter gaps are marked in the native rules and library. The new
read-only replay executes the three Shadow teleports, Inferno Blade, Divine
Impetus, Divine Recovery, Belt of Battle and ordinary full attacks. It does not
write encounter HP, attack rolls, or daily resources to a saved character.

## Original build failures

- True Neutral is prohibited for Crusader (Tome of Battle p.8).
- RKV requires Wee Jas. The local PH deity entry permits LG/LN/LE and grants
  Death, Law and Magic. It does not grant Undeath. A setting/deity adaptation
  must be explicit; it is not silently accepted.
- At character level 12, BAB is 8, below Greater Two-Weapon Fighting's 11.
  Moving that feat to 18 and moving the level-18 Extra Turning to 12 addresses
  the BAB timing issue; ability scores at acquisition still need documentation.
- At character level 17, the Crusader track has IL 12. Greater Divine Surge
  is level 8 and requires IL 15. It cannot be selected there.
- Crinti entry requires the listed skills, Mounted Combat, Stealthy, Dambrath
  region, and either Crinti status or noble sanction. The human build does not
  supply skill spending or sanction evidence.
- Divine Impetus is supernatural, with no alternate activation action stated.
  The default supernatural activation costs a standard action. The original
  sequence spends its standard action on Shadow Jaunt, then fails at its first
  Impetus. Unlimited free Impetus is an explicit counterfactual ruling.
  See [SRD special abilities](https://www.d20srd.org/srd/specialAbilities.htm).

Final BAB 16 is correct. Base saves are Fortitude 11, Reflex 10, Will 12;
with the supplied abilities and resistance +5, the posted 19/26/21 totals agree.
Final IL is **Crusader 14, Swordsage 15**, not 13/15.

The replay isolates the action issue by removing the illegal Greater Divine
Surge selection (leaving that award unused). The free-Impetus probe additionally
assumes the disputed build requirements and the claimed Undeath domain. These
are assumptions, not an automatically repaired character.

## Counterfactual action and damage results

With one Nightstick, four Extra Turning awards including the domain grant,
CHA 22, no prior expenditure, the assumed pool is 29. Two Impetus uses support
the opening four full attacks. Each repeat spends three attempts: Impetus,
Divine Recovery, Impetus. Nine repeats exhaust the remaining 27.

Each full attack has five main-hand attacks including haste and three off-hand
attacks, not four each:

- Main: +35/+35/+30/+25/+20.
- Off: +35/+30/+25.
- Thirteen full attacks: **65 main + 39 off = 104**, not 66 + 39.

Martial Discipline Weapon adds +3 to attack while using its discipline, not to
damage. The physical flat damage with the stated +5 swords is +20 main / +18 off
before precision and Inferno Blade. Inferno adds 3d6+15 fire per hit. With denied
Dexterity and Assassin's Stance, sudden strike contributes 2d6 and sneak attack
2d6; Craven adds 20 only when sneak attack applies. Flanking alone does not
activate sudden strike. Teleportation alone does not deny Dexterity.

| Deadly Precision version | Main / off per hit, before mitigation | All-hit mean over 104 attacks |
|---|---|---:|
| Complete Adventurer (+2 equivalent; +2d6 on sneak attack) | 10d6+55 / 10d6+53 | 9,282 |
| MIC (+1 equivalent; +1d6 on sneak attack or sudden strike) | 9d6+55 / 9d6+53 | 8,918 |

These are conditional averages, not actual damage. Every attack is assumed to
hit; criticals, physical DR, concealment and unreachable vitals are excluded.
The form independently checks precision immunity, fire immunity/resistance,
flanking, denied Dexterity and teleport eligibility. Fire resistance applies
per hit, with the 3d6 distribution evaluated explicitly. The target must be
visible before teleport and within melee reach afterwards. An occupied
teleport destination expends the maneuver but supplies no pounce.

With no Nightstick, the fixed requested sequence stops after 11 full attacks
when attempts run out. An occupied destination or an unseen pounce target
leaves only the Belt's ordinary full attack in this particular sequence.

## Other numerical issues and missing facts

- The listed item prices sum to 709,700 gp, not 739,700. Local Nightstick price
  is 7,500, not 7,200. Further corrections depend on the weapon version and
  the approved custom belt combination.
- Two +5 martial-discipline/MIC-deadly-precision swords are +7 equivalent;
  Complete Adventurer Deadly Precision makes them +8 equivalent. Do not mix
  the MIC price with Complete Adventurer damage. Masterwork/base sword costs
  must also be included.
- The literal Swordsage AC bonus requires light armor. Bracers alone do not
  satisfy that condition. Without a ruling, the supplied items yield AC 33,
  or 34 with haste, rather than the claimed Wisdom bonus.
- Belt of Battle adds +2 initiative: the stated DEX and Swordsage 3 yield +14
  before other effects. Sign has differing source versions; no unsupported
  flat +5 from Sign is certified here.
- HP 154 and the acquisition-time ability/skill history cannot be checked from
  the supplied final scores. Craven qualification through a stance also remains
  an explicitly noted adjudication point.

## Sources and provenance

Local original records under
`data/packs/custom/original/pcgen-BahamutDragon-master/data/35e/wizards_of_the_coast/`:

- `supplement/tome_of_battle/tob_classes.lst`, `tob_abilities.lst`,
  `tob_spells.lst`, `tob_skills.lst`, `tob_equipmods.lst`.
- `supplement/complete_warrior/cw_classes.lst`, `cw_abilities.lst`.
- `core/players_handbook/ph_deities.lst` (Wee Jas).
- `supplement/libris_mortis/lm_equipment.lst` (Nightstick).
- `supplement/magic_item_compendium/mic_equip.lst`, `mic_equipmods.lst`.
- `supplement/complete_adventurer/cad_equipmods.lst`.
- `supplement/spell_compendium/sc_spells.lst`,
  `supplement/miniatures_handbook/mh_spells.lst` (Sign version discrepancy).

Local `shining_south/shs_classes.lst` contains only a commented Crinti stub.
Primary PDF retrieval failed; no visual PDF verification is claimed. The missing
class was supplemented from the [Shining South class transcription](https://www.dndtools.org/classes/crinti-shadow-marauder/).
Readiness tables were cross-checked against [Swordsage](https://www.dndtools.org/classes/swordsage/),
[Crusader](https://www.dndtools.org/classes/crusader/) and
[RKV](https://www.dndtools.org/classes/ruby-knight-vindicator/).
The [original-book excerpt](https://www.scribd.com/doc/49212252/Tome-of-Battle-3-5-OEF)
confirms Crusader's alignment restriction.

## Reproduction

- `cargo test --offline -p pcgen-engine --test martial_case`: one bounded probe,
  covering all class levels first, then the combination and failure boundaries.
- `cargo run --offline -p pcgen-engine --example martial_audit`: machine-readable
  report from the original input fixture.
- `/classes`: complete class tables; `/martial`: English ordinary HTML form
  for read-only stress replay, including a separately labelled house-rule probe.

No full-workspace test gate is required. Clear compiler artifacts after use.

The generated report now includes the shared chronological acquisition audit. Missing original skill allocations and other evidence remain explicit; numerical replay does not certify full build legality. Data coverage, qualification checks and executable features are tracked separately.
