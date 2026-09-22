# 旧实现 → 新架构 迁移说明

> 本文档是 Sprint / 阶段**历史记录**，不再是进度事实源。
> 当前状态、测试数与取舍见 [`ROADMAP.md`](ROADMAP.md)；
> 带日期标注的数字（测试数、差分率）属于当时快照，不要据此判断现在。
>
> 本文中出现的 `17 / 47 / 51 / 53 / 57 / 59 / 60 / 61 / 62` 等测试数
> 均为**对应 Sprint 完成时**的快照，不是当前值；当前值见 `ROADMAP.md`。

## 1. 存档位置

旧 `pcgen-rust` 已完整归档：

```text
reference/pcgen-rust-archive/
```

其中保留：

- 旧的 `pcgen-core` / `pcgen-data` / `pcgen-output` / `pcgen-cli`
- 114 个测试与旧实现
- Java differential 脚本
- 真实 `.pcg` fixture、mini campaign、output sheet fixture
- Gestalt / 灵能 / 导入层 / 诊断 / 补丁模型的首版实现

新 `pcgen-rust` 采用新架构：**先建立统一 IR 与 Draft/Snapshot 边界，再逐步迁移旧能力**。

## 2. 迁移阶段

### M0：架构骨架（已完成）

- 五 crate 新工作区。
- `pcgen-core`：Rational、EntityId、Rule IR、Advancement、Gestalt、SCC/Topo。
- `pcgen-import`：RawRecord、PatchOperation、CompiledRulePack、覆盖度。
- `pcgen-engine`：CharacterDraft、CharacterSnapshot、Engine、ClassProgress。
- `pcgen-output` / `pcgen-cli`。
- 17 个测试。

### M1：真实 PCGen 导入（部分完成）

已完成：

- 从 archive 迁移 `pcc.rs` 与 `tokenizer.rs`。
- `load_pcc`：真实 `.pcc` include 图 + `.lst` lossless Raw AST。
- `compile_pcc`：生成 `CompiledRulePack` 与覆盖度报告。
- 简单 `BONUS:` 降低为 `Effect / Modifier`。
- 简单 `PRESTAT` / `PRECLASS` / `PREVAR*` / `PREABILITY` 降低为 `Predicate`。
- CLI `import-report` 输出 entities / lowered / legacy / unsupported / tag counts。
- 真实 SRD 数据 smoke：约 3,779 entities、23,723 tag occurrences，
  known 19,434（约 82%）、legacy 4,289、lowered effects 2,944、
  unsupported 337。
- `.pcg` Draft 读写与 `GESTALTJSON` 迁移到 `PcgDraft`。
- `pcgen-core::domain` typed definitions：Race / Class / Skill / Spell /
  Equipment / EquipmentModifier。
- `ClassProgression` 由 import 层生成，engine 直接消费
  `CompiledRulePack.progressions`。
- `CompiledRulePack` 携带 typed definitions + progressions。
- `import-report` 输出 typed counts；mini 与真实 SRD typed smoke 通过。
- 覆盖率使用 archive 迁移的 `tokenizer::is_known_tag` 白名单。
- `.MOD / .COPY= / .CLEAR / .FORGET` 记录级补丁语义。
- `DEFINE / VAR / ABILITY / AUTO / CHOOSE` 降低为 `Effect`。
- `BONUS` 公式值降低为 `ModifyFormula`，不再只接受整数。
- `PRESKILL / PREALIGN / PRELEVEL / PRERACE / PRETYPE` 等进一步降低为
  `Predicate`。
- unsupported 从 1,389 降到 337。
- classes 在当前 typed 模型下为 51 个有效职业；archive 的 55 包含把
  numeric class row 当作 class 的历史行为，后续如需逐字段对齐，需要
  单独 diff 这 4 个条目。

待完成：

- 完整 `.MOD` / `.COPY=` / `.CLEAR` / `.FORGET` typed semantics。
- `DEFINE` / `LEVEL` / `ABILITY` / `AUTO` / `CHOOSE` 完整降低。
- 复杂 `BONUS` 公式与全部 `PRE*`。
- 多行 class header 的完整合并与精确 typed count 对齐。

### M2：完整角色评测（已完成首版）

已完成：

- `CharacterSnapshot` 覆盖：
  - 属性 / final stats
  - HP
  - BAB
  - 豁免
  - AC / touch / flat-footed AC
  - armor / shield AC 组件
  - 技能
  - caster level
  - 法术 DC
  - prepared spells
  - 法术位 / known limit
  - 灵能点与显能者等级（首版近似）
- 全读取路径来自 `CompiledRulePack`：typed definitions + progressions。
- `.pcg` Draft 读取扩展：属性、职业、HP、技能、法术、装备、能力、`GESTALTJSON`。
- CLI：
  - `parse-pcg`
  - `recalculate`
  - `export`
- `scripts/differential_java_pcgen.py` 已适配新 JSON。
- 9 个 Java XML 差分全部通过：
  - FigFae / Bard / SWizSam / BrdJoe / WizShar
  - CleElf / BarJack / Quasvin / Ivan（基础字段）

待完善：

- 灵能点目前是首版近似，完整 RSRD BasePowerPoints 表仍在迁移。
- 更复杂的技能加值、装备修正、法术位变体仍未完全迁移。

### M3：Gestalt / prestige / archetype / faith（部分完成）

已完成：

- RSRD 灵能点表精确迁移：
  - Psion / Wilder / Psychic Warrior / Psionic Fist / War Mind
  - ability bonus 与 total power points
- `GestaltPlan` 接入 engine：
  - HP / BAB / 豁免使用 lane 合并结果
  - `snapshot.gestalt` 携带 prestige slot cost、archetype/substitution、
    faith/divine tracks 与诊断
- `.pcg GESTALTJSON` Draft 往返
- archive v1 vs 新 engine snapshot differential 工具
- 恢复 archive 角色解析 golden tests 与 `.pcc` graph tests
- output layer 扩展为 text / Markdown / CSV / JSON / HTML
- coverage non-regression gate：`scripts/coverage_gate.py`

待完成：

- archetype / substitution 的完整替换/移除规则仍未进入 engine 数值计算。
- faith 的完整 FaithSource / FaithDecision 仍主要是 lane 元数据与诊断。
- 声望职业 slot cost 仍是计划/诊断层，未强制到所有运行时路径。
- FreeMarker 子集和完整 output sheet 兼容仍待迁移。

### Sprint C1：Effect 执行引擎（已完成首版）

- 新增 `pcgen-formula` crate，迁移 archive 公式解析/求值。
- 新增 `pcgen-engine::effects::EffectApplicator`：
  - 收集 active entities
  - `Grant / AUTO` 固定点展开
  - `DefineVariable` / `SetVariable` / `DeclareVariable`
  - `Modify` 与 `ModifyFormula`
  - `Choose` 基础执行与 unresolved choices
  - `Predicate` 求值
- 变量通过迭代解析，支持循环/未解析诊断。
- Snapshot 新增：
  - `variables`
  - `external_modifiers`
  - `active_abilities`
  - `choices`
  - `unresolved_choices`
  - `applied_effects`
- Coverage gate 新增：
  - `formula_effects`
  - `grant_effects`
  - `choose_effects`
  - `variable_effects`
- 真实 SRD：
  - formula_effects = 850
  - grant_effects = 604
  - choose_effects = 90
  - variable_effects = 369
- Curated Java XML differential 仍然 9/9 通过。
- 测试数：51。

### Sprint C2：Effect 结果接入与高级规则执行（已完成首版）

- `external_modifiers` 已应用于：
  - HP
  - BAB
  - saves
  - AC / touch / flat AC
  - skills
  - spell DCs
- 结构性 target 去重：
  - class/race 的 `BASEAB` / `BASE.SAVE` / `STAT.*`
  - equipment 的 `COMBAT.AC`
  避免与 typed progression / EquipmentDefinition 重复计算。
- 变量图：
  - 从公式表达式提取依赖
  - 使用 `topological_sort`
  - 循环诊断
- Grant / AUTO：
  - `%CHOICE%` / `%LIST%` 替换
  - granted ability 内嵌 `PRE*` 校验
- Faith / Deity / Domain：
  - `DeityDefinition` / `DomainDefinition`
  - `DEITY` / `DOMAIN` typed parsing
  - engine 校验未知 deity / domain 与 deity-domain 不匹配
- Coverage gate 扩展为执行覆盖率：
  - FigFae: applied_effects >= 10, active_abilities >= 5
  - Bard: applied_effects >= 20, active_abilities >= 10
- Curated Java differential 仍然 9/9 通过。
- 测试数：53。

### Sprint C3：原生 lowerer、Faith 与 Output（已完成首版）

- PCGen 原生 archetype：
  - 从 `TYPE:Archetype` 识别 variant
  - 从 `ABILITY / AUTO` 提取 grants
  - 继续支持 project-level `REPLACES / GRANTS / CSKILL`
- Faith：
  - `DeityDefinition` / `DomainDefinition`
  - `DomainSpell` 从 `SPELLLEVEL:DOMAIN|...` 解析
  - snapshot 增加 `domain_spells`
  - engine 校验：
    - 未知 deity / domain
    - deity-domain 关系
    - alignment 匹配
    - favored weapon 是否在 equipment 中
- FreeMarker 子集扩展：
  - `#assign`
  - `#include`（相对路径）
  - `?size`
  - `?join`
  - `?replace`
  - `?string`
- Broad oracle：
  - 对 custom class / missing data 的 case 标记为
    `data-missing/skipped`，不计入字段一致率。
- 测试数：57。
- Java curated differential 仍 9/9 通过。

### Sprint C4：Substitution / FreeMarker / Oracle 覆盖（已完成首版）

- PCGen 原生 substitution lowerer：
  - `SUBSTITUTIONCLASS:`
  - `SUBSTITUTIONLEVEL:N`
  - 输出为 `VariantDefinition { id = "{substitution} ~ level {N}" }`
  - grants 从 `ABILITY / AUTO` 提取，CSKILL 从 class skills 提取
- FreeMarker：
  - `#macro` / `<@macro/>`
  - `#import ... as ns`（相对路径 include + namespace 前缀剥离）
  - `#include` 保持
- Broad oracle：
  - per-case PCC 映射
  - `35e_Alice` 等 custom case 使用 `pcgen_test_advanced.pcc`
  - data-missing 从 12 降到 5
  - final_stats / initiative 达到 100%
- Coverage gate 字段级执行覆盖：
  - FigFae：applied_effects / active_abilities / skills / variables
  - BrdJoe：applied_effects / active_abilities / spell_dcs / prepared_spells /
    caster_levels
- 测试数：59。
- Java curated differential 仍 9/9 通过。

### Sprint C5：Custom class 公式与字段值门禁（已完成首版）

- PTS custom class 公式：
  - `BONUS:COMBAT|BASEAB|classlevel(...)+3`
  - `BONUS:COMBAT|BASEAB|TL`
  - `BONUS:SAVE|...`
  - 新增 `base_attack_formula` / save formulas
  - engine 运行时按 `class_level + total_level` 求值
  - broad oracle BAB 从 50.0% 提升到 66.7%
- Substitution level：
  - `BONUS:*` 进入 `VariantDefinition.bonus_tags`
  - `ADD:SPELLCASTER` 进入 `spellcaster_advance`
  - snapshot 新增 `variant_effects`
  - variant spellcaster 会推进 caster level
- Broad oracle：
  - per-case PCC 映射
  - data-missing 12 -> 5
  - final_stats / initiative / base_stats / name 达到 100%
- Coverage gate 字段值级差分：
  - FigFae HP / BAB / AC / touch / flat / saves / 关键 skills 精确值
- 测试数：59。
- Java curated differential 仍 9/9 通过。

### Sprint C6：公式变量、Substitution Effect 与全量差分门禁（已完成首版）

- Class 公式中的更多 PCGen 变量语义：
  - `var("X")` 读取 effect 变量
  - `var("BL=Class")` 使用 class level
  - `classlevel(...)` / `TL` / `CL` 继续支持
- Substitution effect 真正执行：
  - `VariantDefinition.effects: Vec<Effect>`
  - `BONUS:*` 会 lower 成 IR effect
  - EffectApplicator 对 Modifier / ModifyFormula / DefineVariable /
    SetVariable / DeclareVariable 执行
  - `ADD:SPELLCASTER` 提升 caster level
- FreeMarker：
  - macro 默认参数
- Coverage gate：
  - FigFae 字段值精确校验
  - BrdJoe 执行字段覆盖
  - 直接运行 9 个 curated Java differential
- Broad oracle：
  - data-missing 5
  - final_stats / initiative / base_stats / name 100%
  - BAB 66.7%
- 测试数：60。
- Java curated differential 仍 9/9 通过。

### Sprint C7：Namespace Macro、Variant Action 与更多字段值门禁（已完成首版）

- Substitution / variant action 扩展：
  - Grant / Choose 进入 variant effect 固定点循环
  - granted ability 可继续展开其 effects
  - `%CHOICE%` / `%LIST%` 可用于 variant grant
- FreeMarker namespace：
  - `#import "file" as ns` 保留 `ns.macro` 命名空间
  - `<@ns.macro/>` 可解析
  - 指令支持 `>` 与换行两种终止
- 官方风格 fixture：
  - `pcgen_official_subset.ftl` + `#include` + `#macro` smoke
  - macro 在 `#list` 内精确等价仍是已知缺口
- Coverage gate：
  - Bard 字段值精确断言：
    HP / BAB / AC / touch / flat / saves / skills
  - 继续运行 curated Java differential
- Broad oracle：
  - data-missing 5
  - final_stats / initiative / base_stats / name 100%
  - BAB 66.7%
- 测试数：61。
- Java curated differential 仍 9/9 通过。

### Sprint C8：FreeMarker macro-in-list 与官方 golden（已完成首版）

- FreeMarker：
  - 支持 `</#list>` / `</#if>` 闭合形式
  - macro 调用可在 `#list` 内精确展开
  - `#import as ns` 命名空间 macro
  - `#assign` / `#include` 同时支持 `>` 与换行终止
- 官方风格 fixture：
  - `pcgen_official_subset.ftl` + `#include` + `#macro` + `#list`
  - 与 `pcgen_official_subset.xml` 精确 golden 一致
- Coverage gate 继续包含：
  - FigFae / Bard 字段值精确断言
  - 9 个 curated Java differential
- 测试数：61。
- Java curated differential 仍 9/9 通过。

### Sprint C9：真实官方 Sheet、Uncanny Dodge 与更多测试（已完成首版）

- FreeMarker / output：
  - `#else` / `</#if>` / `</#list>` 支持
  - FTL 注释与残留指令清理
  - macro 在 `#list` 内精确展开
  - `pcgen_official_subset.ftl` exact golden
  - 真实官方 `csheet_5e_xml_generic.html.ftl` smoke：
    - 保留 `<character>`
    - 不再泄漏 `<#`、`${`、`</@`
- Rules：
  - Uncanny Dodge / Retain Dex 影响 flat-footed AC
  - Broad flat_footed_ac 66.7% -> 72.2%
- Tests：62。
- Java curated differential 仍 9/9 通过。

### M4：覆盖度与 CI（已完成首版）

> 测试/差分数字为当时快照；当前数字见 `ROADMAP.md`。

已完成：

- Java curated differential：9/9 通过。
- Broad Java differential oracle：23 个 case 字段级统计。
- archive v1 vs 新 engine snapshot differential 工具。
- `scripts/coverage_gate.py`：
  - raw import coverage（entities / lowered / legacy / unsupported）
  - typed definitions counts（races / classes / skills / spells / equipment /
    equipment modifiers / progressions）
  - 已接入 `scripts/check.sh` 与 CI。
- 测试数：47。

broad oracle 当前字段一致率：

```text
name            100.0%
base_stats      100.0%
race             82.6%
classes          82.6%
final_stats      87.0%
initiative       91.3%
reflex/will      87.0%
bab              73.9%
armor_class      65.2%
touch_ac         78.3%
hit_points       65.2%
flat_footed_ac   56.5%
fortitude        60.9%
```

低一致率主要来自 broad oracle 中需要使用角色专属数据源/custom class 的 case，
curated 9 个 case 仍然全部通过。

### M3 补充：variant / faith / prestige / template

- `VariantDefinition`：archetype/substitution 的 replaces / grants /
  class_skills 替换规则。
- Engine 通过 `features_for_class` / `class_skills_for_lane` 应用 variant。
- `FaithDecision` 与 `FaithPolicy`：divine tracks 按 Enforce/Warn/Ignore
  产出 allowed / warnings / errors，并进入 snapshot。
- Prestige slot cost 在 `Engine::validate` 中给出 overflow / mirrored-lane
  诊断。
- 最小 FreeMarker 风格模板：
  - `${name}`、`${hp}`
  - `#list skills as skill`
  - `#if hp > 10`
  - `?upper_case` / `?lower_case`

## 3. 迁移规则

1. 旧行为先用测试冻结，再迁移。
2. 迁移后必须删除旧运行路径，不能长期双轨。
3. 新功能只允许进入新 IR，不允许再往旧 `DataStore` 路径加规则。
4. 每个 tag family 迁移必须同时增加覆盖度统计和 fixture。
