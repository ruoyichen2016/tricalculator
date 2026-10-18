# Warforged / Umbral Disciple extreme-case audit

The post's totals are not a legal-build certificate. Standard multiclass BAB is
**15**, not 16. The pool is **14 essentia** after counting Bonus Essentia once;
the second selection is invalid. The proposed initial Tiger Claw choices also
fail their maneuver prerequisites. The corrected action probe is explicitly
separate from the original input.

## Complete class levels before combination

The focused test first traverses Ranger 1–20, Sneak Attack Fighter 1–20,
Totemist 1–20, Umbral Disciple 1–10, Warblade 1–20 and Incandescent Champion
1–10: **100 levels**. It then evaluates the chronological multiclass history.

Ranger retains the existing complete RSRD progression. Sneak Attack Fighter is
a distinct variant: all fighter bonus feats are removed, and sneak attack
advances at every odd fighter level. It is not implicitly the Thug variant.
The four newly introduced classes have complete class chassis, feature
acquisition records and numerical progression tables. Totemist's missing
base soulmeld count and Umbral Disciple's incomplete local chassis are repaired
using the original book's indexed table text.

Incarnum class tables are independent of martial progression and spell slots.
Umbral Disciple and Incandescent Champion add essentia but do not advance this
character's Totemist meldshaper level, soulmeld count or chakra binds. At 20 the
character still has meldshaper level 2, three soulmelds (subject to CON), one
bind, and only the Totem chakra open. The normal investment cap is 4; a soulmeld
bound to Totem has cap 5. At character 15 those caps are 3 and 4.

**Execution boundary:** complete level tables and descriptions are not the same
as implementing every encounter effect of every class level. The shared
incarnum evaluator implements this case's pool, capacity, allocation and
Totem-bind restrictions. Its action replay supports Blink Shirt, Sudden Leap,
Pouncing Charge, Dancing Mongoose, White Raven Tactics on another ally, normal
full attacks, reallocation and a start-of-turn Fast Healing tick. Four soulmelds
are recognized: Blink Shirt, Wormtail Belt, Totem Avatar and Girallon Arms.
Other binds and effects are explicitly rejected or marked descriptive. No
encounter HP, saved character or shared daily resource state is mutated.

## Chronological findings

| Character level | BAB | Essentia after one Bonus Essentia | Sneak attack | Ordinary full attack opportunities |
|---|---:|---:|---|---:|
| 1 | 1 | 0 | — | 2 |
| 4 | 3 | 1 | 1d6 | 4 |
| 5 | 4 | 2 | 1d6 | 4 |
| 12 | 9 | 8 | 3d6 | 6 |
| 15 | 11 | 11 | 4d6 | 8 |
| 17 | 12 | 12 | 4d6 | 8 |
| 20 | 15 | 14 | 4d6 | 8 |

These attack counts assume an available body slam even with occupied hands;
the form makes this assumption explicit and allows it to be disabled. Level 1
uses one weapon and a shield; subsequent levels use two light weapons, with
Gloves of the Balanced Hand from level 12. No haste, soulmeld attacks or boost
is included in this table. All attacks require a full attack, not a standard
attack action.

At level 20 BAB is 2 + 1 + 1 + 7 + 2 + 2 = 15. This gives three main-hand
iteratives and two off-hand attacks with the gloves, plus slam, Second Slam and
Jaws of Death: **5 manufactured + 3 natural = 8**. Natural weapons do not gain
BAB iteratives. If the slam is unavailable, Second Slam is unavailable too,
leaving six. Without the gloves the assumed routine has seven. The second slam
uses the feat's highest-attack-minus-five rule, not another iterative series.

A fractional-BAB campaign would instead obtain floor(16.25) = 16, but that is a
separate optional rule and is not silently enabled. This explains a possible
source of the post's BAB claim.

Bonus Essentia has no repeatable/stacking exception. A meldshaper receives two
points once: Totemist 2 + Umbral Disciple 7 + Champion 3 + feat 2 = 14. Repeating
the feat does not produce 16. The level-18 feat slot remains unchosen in the
probe rather than being silently repaired with an invented benefit.

Warblade IL 9 at character 17 and IL 11 at character 20 are correct. However,
Sudden Leap needs one prior Tiger Claw maneuver, and Dancing Mongoose and
Pouncing Charge each need two. Leading The Charge is White Raven, so the
listed choices cannot bootstrap each other. Warblade 2 grants a fourth maneuver
known, not a new stance: IL 11 does not itself award a sixth-level stance.

The default repair replaces Dancing Mongoose with Wolf Fang Strike and learns
it before Sudden Leap and Pouncing Charge. It retains Leading The Charge and
learns White Raven Tactics at 20. All four known maneuvers cannot be readied
simultaneously: the Warblade 2 limit is three.

A separate test uses Wolf Fang Strike, then Blood In The Water, Dancing
Mongoose and Pouncing Charge. It can produce **10** attacks, or **12** with two
free Girallon claws, under the stated equipment/slam assumptions. It gives up
the original Leap/White Raven combination. Girallon attacks are a specific
bound-meld benefit, not a generic extra attack from having any Totem bind.

## Resource and action boundaries

- All investments consume the same pool. Each target must also satisfy its own
  capacity. Over-allocation, large integer overflow attempts, unacquired class
  features, duplicate shaped melds and multiple soulmelds on Totem are rejected.
- Blink Shirt costs a standard action, or a move action when bound to Totem.
  Either version ends further actions until the next turn. It is not a way to
  teleport and then full attack. Range is 10 + 10 per invested point.
- Essentia reallocation, Sudden Leap, Dancing Mongoose and White Raven Tactics
  compete for the same swift action. Sudden Leap supplies movement, not another
  action; its distance depends on the Jump result and running start, and the
  movement can provoke attacks of opportunity.
- Pouncing Charge requires its readied maneuver and a full-round action, plus a
  valid charge path. It is not permanent pounce. The probe assumes speed 30 and
  checks a supplied clear path and 10–60 feet of movement; it does not solve map
  geometry. Target distance is measured at the attack.
- White Raven Tactics changes another ally's initiative and may give that ally
  another turn. It does not grant this actor a swift action or reset this
  actor's resources. Self-target use is explicitly outside the strict probe.
- Kiss of the Shadows with 3 essentia reaches 20 feet **on the actor's turn**.
  Outside that turn, threatened reach remains 5 feet. The adjacent-square rule
  still determines flanking; reach alone does not grant sneak attack.
- Soulchilling Strike is consumed by the first successful attack of the round,
  even if the target saves. Further hits do not apply it again. Its ledger
  accepts an already-used-this-round flag so a later turn in the same round
  cannot reset it. Fortitude DC is 10 + investment + CON modifier.
- Staggering Strike needs a damaging melee sneak attack. Each eligible hit can
  prompt a Fortitude save against actual damage dealt; successful saves, natural 1/20 and
  immunities matter. Multiple applications do not stack. The helper reports
  success against supplied rolls; condition expiry, Heal checks and magical
  healing are not persisted by this read-only replay.
- Totem Avatar's invested damage bonus requires its Totem bind and applies only
  to natural weapons. Its unbound investment improves natural armor. It and
  Wormtail Belt's enhancement do not stack. Wormtail Belt's Totem stinger is
  the only attack permitted that round, so it cannot be added to this routine.
- Champion 3 provides at most three Fast Healing rounds per day. A tick occurs
  at turn start with the effect already active, not repeatedly during a turn.
  The ECS living-construct rule halves supernatural healing, rounded down;
  Champion Fast Healing is labelled Su. Thus investment 1 heals 0 under this
  source rule; investment 2 heals 1. The form shows the nominal rate and actual
  tick separately. Ordinary extraordinary fast healing is not universally
  halved by this implementation.

## Missing facts and defenses

The post supplies neither ability scores nor alignment nor skill expenditures.
The probe explicitly assumes NG and CON 14; it cannot certify prestige entry
skills or historical feat ability thresholds. Umbral entry requires
Concentration 4, Hide 8, Knowledge (arcana) 2, Listen 8 and one essentia.
Champion requires good alignment, BAB 6, Concentration 4 and one essentia.
The normal class-entry API now checks the actual pre-entry essentia pool rather
than relying on a manually asserted active state.

Mithral Body supplies +5 armor, maximum DEX +5, an armor check penalty and light
armor status. Ranger style therefore remains usable. Exact AC and HP need the
missing scores and equipment. Battle Clarity is conditional and capped by both
INT bonus and Warblade level; +1 is not guaranteed without the INT score.
Entering Totemist does not erase existing literacy. Living construct traits
are not full construct immunity: the character remains subject to critical
hits, sneak attack, stunning, nonlethal damage and mind-affecting effects.
Light Fortification negates critical/sneak extra damage only 25% of the time.

ECS and Monster Manual III word their healing traits differently. This probe
explicitly uses the local ECS supernatural-healing clause; no silent source
mix is presented as an uncontested universal fast-healing rule.

## Sources and reproduction

Direct local original data under
`data/packs/custom/original/pcgen-BahamutDragon-master/data/35e/wizards_of_the_coast/`:

- `supplement/magic_of_incarnum/moi_classes.lst`, `moi_abilities.lst`, `moi_feats.lst`.
- `supplement/tome_of_battle/tob_classes.lst`, `tob_abilities.lst`, `tob_spells.lst`.
- `campaign_settings/eberron/campaign/ecs_abilities.lst`, `ecs_feats.lst`.
- `supplement/complete_adventurer/cad_feats.lst` (Staggering Strike).
- `supplement/magic_item_compendium/mic_equip.lst` and
  `campaign_settings/forgotten_realms/magic_of_faerun/mag_equip.lst` (gloves).

Missing tables and feat text were checked against search-indexed **original
book** text: [Magic of Incarnum](https://www.rpg.korliane.org/DnD/Magic%20of%20Incarnum.pdf)
pp.19, 29, 35, 115, 158;
[Tome of Battle](https://dtdnd.neocities.org/books/player/Tome%20of%20Battle.pdf)
p.22; [Races of Eberron](https://www.rpg.korliane.org/DnD/Supplements/Races/Races%20of/Races%20of%20Eberron.pdf)
pp.119–120 (Jaws of Death, Second Slam).
The [UA fighter variant](https://www.d20srd.org/srd/variant/classes/variantCharacterClasses.htm)
removes all fighter bonus feats. PDF download/whole-document access failed;
no visual page verification is claimed. Forum answers were not used as rules.

- Input: `tests/fixtures/forum-builds/warforged-umbral.json`.
- Generated report: `tests/fixtures/forum-builds/warforged-umbral-audit.json`.
- Reproduce: `cargo run --offline -p pcgen-engine --example incarnum_audit`.
- Targeted check: `cargo test --offline -p pcgen-engine --test incarnum_case`.
- English ordinary forms: `/incarnum`; complete progression tables: `/classes`.

The new test runs all 100 class levels before the combination and edge cases.
The previous Shadow Pounce test is the only additional regression probe; no
full-workspace gate is needed. Compiler outputs are cleared after installation.

The generated report now includes the shared chronological acquisition audit. Missing original skill allocations and other evidence remain explicit; numerical replay does not certify full build legality. Data coverage, qualification checks and executable features are tracked separately.
