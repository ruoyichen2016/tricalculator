# Tag 迁移工作清单

> 由固定数据上的真实 3e SRD（`srdbasic.pcc`）生成。
> 最近更新：P4 全量关闭后。legacy pending 与 unsupported pending 均为 0。

## 0. 口径

三个互斥桶：allowlisted（不计入分母）/ pending legacy / unsupported。

```text
lowered_effects     3267
legacy_tags         1284
  ├─ allowlisted    1284
  └─ pending        0
unsupported_tags    0
  ├─ allowlisted    0
  └─ pending        0
semantic_pending    0
semantic_completion 100.0%
```

## 1. pending unsupported

无。三个验收数据集的 unsupported pending 均为 0。

## 2. pending legacy（0 tag，0 次）

无。DS-3E/DS-35E/DS-PSI 的 legacy pending 均为 0。

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
| 29 | `SOURCEWEB` |
| 28 | `SOURCEDATE` |
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
| 5 | `XPCOST` |
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

累计：`legacy_tags` 4289 → 1284，`unsupported` 337 → 0，`semantic_completion` 46.8% → 100.0%。
