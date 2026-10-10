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
- **Post-M9**：`PREREGION` → `Predicate::Region`；模板 `REGION:`/`SUBREGION:`
  落到 `CreatureTraits.region`/`.subregion`，不再计入 legacy。验收数据集不含该
  tag，cohort 数字不变；第三方资料 unsupported 1,171→750（见
  `THIRD_PARTY_TRANSCRIPTION.md`）。
- **Post-M9**：`PRECSKILL`/`!PRECSKILL` → `Predicate::ClassSkill` +
  `SkillSpec`；`EffectState.class_skills` 按角色职业构建（含 archetype /
  substitution level 换入的 class skill）。第三方资料 unsupported 750→401
  （含 319 条以 `PRECSKILL` 为 gate 的 `BONUS`）。未建模：模板/种族/专长上的
  全局 `CSKILL:`/`CCSKILL:` 与 `SERVESAS`。
- **Post-M9**：`PREALIGN` 的两种取值形态——数值 key 不再被丢弃（保留为 fail-closed
  的 `Alignment`，而不是整条前置消失），`PREALIGN:Deity` 实现 `alignMatches` 的
  「角色阵营 == 神祇阵营」特例。第三方资料 unsupported 401→393。
- **Post-M9**：`PRECHECK` → `Predicate::Check`（新增 `EffectState.total_saves` =
  base + 属性调整）；同时修掉 `PRECHECKBASE` 丢掉 count 的老问题（`BaseSave` 保留以
  兼容旧 pack）。第三方资料 unsupported 393→373。
- **Post-M9**：`PRESPELLDESCRIPTOR` → `Predicate::SpellDescriptor`（新增
  `EffectState.spell_descriptors`；多组 `y=z` 是**相加**，与 `PRESPELLTYPE` 的「任选其一」
  不同）。第三方资料 unsupported 373→350。
- **Post-M9**：`PRESPELLSCHOOLSUB` → `Predicate::SpellSchoolSub`（与
  `PRESPELLDESCRIPTOR` 同构，新增 `EffectState.spell_subschools`）。第三方资料
  unsupported 350→341。
- **Post-M9**：`PRESPELLSCHOOL` 语义修复（非 unsupported 项）：新增
  `EffectState.spell_schools`，按法术 `SCHOOL:` 匹配并保留 count / 配对相加；旧
  `SpellSchoolLevel` 仍可反序列化但求值改用修正后的匹配。第三方 504 条受影响。
  见 `PCGEN_SYNTAX_INVENTORY.md` §5z。
- **Post-M9**：`PRECAMPAIGN` → `Predicate::Campaign`；`CompiledPackHeader` 新增
  `campaigns`（由 `.pcc` 图填充，根为「已选中」）。第三方资料 unsupported 341→332。
  注意这 9 条在职业等级行上，而 CLASS_LEVEL 的 requirements 目前不被求值（§5aa）。
- **Post-M9**：`PRECHARACTERTYPE` → `Predicate::CharacterType`；`CharacterDraft`
  新增 `character_type`（默认 `PC`，`.pcg` 的 `CHARACTERTYPE:` 会被读入）。第三方资料
  unsupported 332→311（连带 `BONUS` 280→268）。
- **Post-M9**：`PRESA` → `Predicate::SpecialAbility`；同时把此前**静默丢弃**的
  `SA:`/`SAB:` 建模为 `EffectAction::GrantSpecialAbility`（第三方 2378 条，按 key name
  前缀匹配，支持 PRE 限定与 `.CLEAR`）。第三方资料 unsupported 311→304。
- **Post-M9**：`PREFACT` → `Predicate::Fact`，新增 `FACT` 模型（`CompiledEntity.facts`
  + `EffectState.location_facts`，只 lower `abilities`/`race` 两个可枚举 location）。
  第三方资料 unsupported **304→42**（`BONUS` 268→23、`PREMULT` 19→2），是本循环最大一次下降。
- **Post-M9**：`PRESPELLTYPE` 的法术类型改为**自由文本**（`SpellTypeSpec.spell_type`
  用 serde `alias = "power"` 兼容旧 pack；`spell_slots_by_type` 取代
  `slots_by_power_level`；未声明 `FACT:SpellType` 的职业被跳过）。第三方 unsupported
  23→5。
- **Post-M9**：`PRESPELLCAST` 语义修复：每个逗号 token 都读（多 token = 任一成立），
  法术类型改为**文本**比较（`HasSpellType`，`ClassDefinition.spell_type` 取
  `FACT:SpellType` 原文，未声明为 `None`）。第三方 unsupported 25→23，`PREMULT` 归零。
- **Post-M9**：`PREUATT` → `Predicate::HighestClassBab`（reference 名为「非武装攻击加值」，
  实为**各职业 BAB 的最大值**；新增 `EffectState.highest_class_bab` /
  `bab::max_class_bab`）。第三方资料 unsupported 29→25。
- **Post-M9**：`PRESKILLTOT` → `Predicate::SkillTotal`（复用 `SkillSpec`；按 reference
  把匹配技能的 ranks **求和**，`TYPE.Text` 汇总该类型全部技能，总数取最后一个 `=`，
  无 `=` 时为 0）。第三方资料 unsupported 32→29。
- **Post-M9**：`PRECLASSLEVELMAX` → `Not(ClassRequirement{…, min_level=z+1})`
  的 `AtLeast`（即 reference 里对 `PRECLASS` 的反写），并把 `class_spec_from` 抽出复用。
  同时记录 `PRECLASS` 的 `TYPE=` 聚合分歧（max vs sum，见 §5ai）。
- **Post-M9**：`PREITEM` → `Predicate::Item`（名字含 `%` 通配 / `TYPE=` 点分
  类型列表，按**持有**而非已装备计数）；同时修掉旧实现把它 lower 成 `Predicate::Equipped`
  且只认 `TYPE=` 的错误。新增 `EffectState.item_types`。
- **Post-M9**：`PREREACH`/`PREREACHGTEQ` 等 7 个 kind →
  `Predicate::ReachAtLeast` / `ReachAtMost`（按 size 家族的组合写法），新增
  `EffectState.reach_feet`（种族 REACH，默认 5；模板覆盖；`BONUS:COMBAT|REACH` 相加）。
- **Post-M9**：`PREVAR*` 语义修复（非 unsupported 项）：
  `VariableCompare` 新增 `strict` 以区分 GT/GTEQ 与 LT/LTEQ 的边界（第三方 2519 对
  符号操作数受影响），`variable_value` 增加字面量回退。
- **Post-M9**：`PREDR` → `Predicate::DamageReduction`，新增
  `EffectState.damage_reduction`（非加值 DR）+ `BONUS:DR` 加值合并。
- monster class `classlevel()` BAB 公式修复（`classlevel()` 空括号）

累计：`legacy_tags` 4289 → 1284，`unsupported` 337 → 0，`semantic_completion` 46.8% → 100.0%。
