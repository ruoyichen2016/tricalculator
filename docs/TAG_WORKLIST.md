# Tag 迁移工作清单

> 由固定数据上的真实 3e SRD 生成。

## 0. 口径

三个互斥桶：allowlisted（不计入分母）/ pending legacy / unsupported。

```text
lowered_effects     3108
legacy_tags         2773
  ├─ allowlisted    1284
  └─ pending        1489
unsupported_tags    3
semantic_pending    1492
semantic_completion 67.6%
```

## 1. pending unsupported（1 tag，3 次）——建模缺口

| count | tag |
|---:|---|
| 3 | `BONUS` |

> 剩余 3 个 BONUS 的条件使用装备类谓词（`PREITEM`/`PREAPPLY`/`PREARMORTYPE`），
> 待 `P4-EQP-1` 一并清零。

## 2. pending legacy（50 tag，1489 次）

| count | tag |
|---:|---|
| 172 | `DOMAINS` |
| 164 | `VISION` |
| 156 | `TEMPBONUS` |
| 145 | `REACH` |
| 136 | `NATURALATTACKS` |
| 90 | `CHOOSE` |
| 66 | `CR` |
| 64 | `EQUIPBUY` |
| 59 | `SKILLMULT` |
| 58 | `CONTAINS` |
| 39 | `DEFINESTAT` |
| 33 | `TEMPVALUE` |
| 23 | `LEVELSPERFEAT` |
| 22 | `SPELLLEVEL` |
| 20 | `UDAM` |
| 17 | `SLOTS` |
| 17 | `STACK` |
| 16 | `LOCATION` |
| 16 | `PROHIBITSPELL` |
| 14 | `DR` |
| 13 | `SR` |
| 11 | `APPLY` |
| 11 | `CHARGES` |
| 11 | `RACESUBTYPE` |
| 10 | `VARIANTS` |
| 9 | `ABILITYLIST` |
| 9 | `SIZENUM` |
| 8 | `ADDSPELLLEVEL` |
| 8 | `CHOICE` |
| 8 | `KNOWNSPELLSFROMSPECIALTY` |
| 8 | `REACHMULT` |
| 6 | `STATMOD` |
| 5 | `XPCOST` |
| 4 | `ALTCRITMULT` |
| 4 | `ALTCRITRANGE` |
| 4 | `ALTDAMAGE` |
| 4 | `ALTEQMOD` |
| 4 | `ALTTYPE` |
| 4 | `ARMORTYPE` |
| 4 | `BASEQTY` |
| 3 | `EXCLASS` |
| 3 | `HITDIE` |
| 2 | `VALIDFORDEITY` |
| 2 | `VALIDFORFOLLOWER` |
| 2 | `WEAPONBONUS` |
| 1 | `ADDLEVEL` |
| 1 | `ATTACKCYCLE` |
| 1 | `DEITYWEAP` |
| 1 | `SPELLBOOK` |
| 1 | `UMULT` |

## 3. allowlisted（47 tag，1284 次）

| count | tag |
|---:|---|
| 180 | `ASPECT` |
| 138 | `LEGS` |
| 122 | `SPROP` |
| 109 | `TEMPDESC` |
| 59 | `MONCSKILL` |
| 59 | `MONSTERCLASS` |
| 55 | `MONNONSKILLHD` |
| 52 | `HANDS` |
| 38 | `MODS` |
| 36 | `EDITABLE` |
| 36 | `EDITPOOL` |
| 36 | `HITDICEADVANCEMENT` |
| 35 | `DISPLAYLOCATION` |
| 33 | `ASSIGNTOALL` |
| 33 | `FRACTIONALPOOL` |
| 31 | `SOURCEWEB` |
| 30 | `SOURCEDATE` |
| 26 | `ABB` |
| 21 | `MODTOSKILLS` |
| 19 | `FORMATCAT` |
| 18 | `MONSKILL` |
| 15 | `DATAFORMAT` |
| 12 | `QUALITY` |
| 11 | `DISPLAYNAME` |
| 11 | `PLURAL` |
| 10 | `BENEFIT` |
| 9 | `REMOVABLE` |
| 8 | `SERVESAS` |
| 7 | `COMPANIONLIST` |
| 6 | `EXCLUSIVE` |
| 4 | `SELECTABLE` |
| 3 | `EXPLANATION` |
| 3 | `FOLLOWERS` |
| 3 | `REQUIRED` |
| 2 | `GENDERLOCK` |
| 2 | `GROUP` |
| 2 | `ITEMCREATE` |
| 1 | `COPYMASTERBAB` |
| 1 | `COPYMASTERCHECK` |
| 1 | `COPYMASTERHP` |
| 1 | `FREE` |
| 1 | `ISDEFAULTSIZE` |
| 1 | `NUMPAGES` |
| 1 | `PAGEUSAGE` |
| 1 | `SITUATION` |
| 1 | `SUBRACE` |
| 1 | `USEMASTERSKILL` |

## 4. 已消费/已 lower

- **ABILITY `CATEGORY:`** → `CompiledEntity.ability_category`（ability pool）
- **EQUIPMOD `PLUS`/`ITYPE`/`REPLACES`** → `EquipmentModifierDefinition` 字段
- **EQUIPMENT `CRITMULT`/`CRITRANGE`/`WIELD`/`DAMAGE`** → `EquipmentDefinition` 字段
- BONUS 家族（size/PRERULE/legs）、PRE 家族（PREMULT/PRERACETYPE/PREAGESET）
- **P4-PRE-1**：`PREVARNEQ` → `Not(Variable)`；`PREPCLEVEL` → `CharacterClassLevel`，
  残余 `PREMULT` 清零（unsupported 38→35）
- **P4-PRE-2**：`PRETOTALAB` → `TotalBab`；`compute_bab()` / `derive_prereq_bab()`
  统一 class BAB 与最终 snapshot BAB（unsupported 35→25）
- **P4-PRE-3**：`SPELLTYPE` 模型落地——`PRESPELLTYPE` → `SpellType`、
  `PRESPELLBOOK` → `HasSpellbook`、`PRESPELLCAST` → `CastingTypeIs`/`HasSpellPower`；
  DS-3E unsupported 25→16（`PRESPELL*` 清零）
- **P4-PRE-6（部分）**：`PRESIZEEQ`/`PRESIZEGTEQ`/`PRESIZELT`/`PRESIZELTEQ`/`PRESIZEGT`
  → size 谓词，`PRETEXT` 改判 allowlist
- **P4-PRE-4**：`PRETEMPLATE`/`PREHANDSGTEQ`/`PREHANDSEQ`/`PREMOVE` →
  `TemplateCount` / `Hands*` / `MovementAtLeast`；DS-3E unsupported 16→3
- **P4-PRE-5**：顶层 `!PRExxx` 统一取反为 `Not(..)`；DS-3E 的
  `!PRERACE`/`!PRETYPE`/`!PREABILITY`/`!PRECLASS`/`!PREAGESET` 清零
  （legacy_tags 2,902→2,773，semantic_completion 65.4%→67.2%）
- monster class `classlevel()` BAB 公式修复（`classlevel()` 空括号）

累计：`legacy_tags` 4289 → 2773，`unsupported` 337 → 3，`semantic_completion` 46.8% → 67.6%。
