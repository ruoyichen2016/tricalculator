# Changelog

## Unreleased

### P0 工程收口（已完成）

- 仓库初始化 `git` 并提交当前 117 测试基线；`.gitignore` 排除
  `target/`、`.cargo-home/` 等生成物。
- 恢复 `../raw/pcgen-master/.pcgen-rust-testdata` marker；
  `coverage_gate.py` provenance 从 `unknown` 变为固定 lock ref。
- `ROADMAP.md` P0 关闭，阶段 0 口径冻结完成。

### P4-KIT-1 KIT / STARTPACK 声明式 typed 导入（已完成）

- `pcgen-core`：新增 `KitDefinition` / `KitEntry`，并加入
  `TypedDefinitions.kits` 与 `CompiledRulePack.kits`。
- `pcgen-ir`：解析 KIT/STARTPACK/TABLE 行，保留 `OPTION` / `LOOKUP` /
  `VALUES` / `QTY` / `EQUIPBUY` / `APPLY` / `LOCATION` 等表达式；
  `classify_for_category(category, tag)` 区分 KIT 与 Equipment 同名 tag，
  KIT tag 由 typed import 消费。
- 真实数据：DS-35E KIT pending 6,896→0，semantic completion
  45.5%→76.2%；DS-3E 67.6%→68.9%；测试 119→122。
- broad oracle / archive differential 行为不变（KIT 不执行随机展开）。

### P4-BONUS/EQP 第二阶段 + P4-SPELL-1 S1 + P4-RACE/TPL 分类（已完成当前切片）

- 残余 `BONUS` unsupported 清零；新增 `PREEQUIP*` / `PREWEAPONPROF` /
  `PREWIELD` / `PREDOMAIN` / `PREHD` / `PREVAR*` symbolic / `PREARMORTYPE` /
  `PREPROFWITHARMOR` / `PREPROFWITHSHIELD` / `PRESPELL` / `PRESPELLSCHOOL` /
  `PRESUBCLASS` / `PRELANG` predicate 解析与求值。
- 装备 typed 字段：`BASEITEM` / `CONTAINS` / `SLOTS` / `LOCATION` /
  `CHARGES`；DS-3E/DS-35E 这些 pending 清零。
- AC same-type non-stacking / dodge/circumstance/untyped stacking 切片；
  `TYPE=NaturalArmor.STACK` 保留 stack 语义。
- Spell/domain typed/消费：`DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` /
  `ADDSPELLLEVEL` / `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK`。
- Race/Template `CreatureTraits` typed：reach / natural attacks / DR / SR /
  CR / vision / subtypes / move clone 等；TemplateDefinition 加入
  `TypedDefinitions` 与 `CompiledRulePack`。
- 真实数据：DS-3E unsupported pending 0，completion 84.2%；
  DS-35E unsupported pending 0，completion 88.3%；broad oracle 23/23 100%。
- 测试 122→123。

### P4-SPELL-1 预研（已完成）

- 新增 [`docs/P4_SPELL1_PRERESEARCH.md`](docs/P4_SPELL1_PRERESEARCH.md)：
  按 category 统计 `DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` /
  `ADDSPELLLEVEL` / `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK` 与
  `PRESPELL` / `PRESPELLSCHOOL`，给出 S1 typed 定义 / S2 执行的分阶段验收。
- 预研不改变 engine 行为，可与 P4-BONUS/EQP 第二阶段并行。

### P4-RACE-1 / P4-TPL-1 / P4-DATA-1 第一阶段（已完成）

- `.pcg` 解析 `TEMPLATESAPPLIED`，Template entities 可激活。
- ability `.MOD` 合并到 base entity；`PRE*` 内嵌条件保留；`PRERACE` /
  `HasEntity` 支持 race/class。
- Halfling racial trait default 链修复，`3e_SWizSam` 三项豁免与 Java exact。
- Half-Dragon template 的 `BONUS:STAT`、natural AC 与 HP 生效，
  `35e_CloudGiantHalfDragon` / `35e_JimDop` 全字段与 Java exact。
- `CLASSABILITIESLEVEL:HITPOINTS` 按等级保留，HP 使用 per-level minimum；
  `3e_MonKee` HP 21→23。
- broad oracle：23/23 java-compared + 0 known-diverging，全部字段 100%。

### P4-ENG-1 类/等级实体激活与 modifier target 归一化（已完成）

- `pcgen-core`：`AttackProgression` 增加 `None` 并作为默认；没有显式
  `BONUS:COMBAT|BASEAB` 的职业不再获得默认半等级 BAB。
- `pcgen-engine`：class/class-level 实体激活与 progression lookup 统一
  stable key / display name（`charlie_class` ↔ `Charlie Class`）；
  `HP.CURRENTMAX`、`TL` 公式和 BASEAB/BASE.SAVE 去重归一化。
- 回归测试：`pcgen-engine/tests/effects.rs` 新增无 BASEAB 职业、带空格类 key、
  `HP.CURRENTMAX`、BASEAB/BASE.SAVE 不重复计数 3 个用例；测试 114→117。
- broad oracle：BAB 13/21→21/21（100%）；AC 17→20、touch 17→19、
  flat-footed 16→19、HP 15→17；`java_gated` 18→21；broad baseline 按
  Java 一致率收敛后重生成。

### P4-KIT-0 KIT / STARTPACK 设计 spike（已完成）

- 新增 [`docs/P4_KIT0_DESIGN.md`](docs/P4_KIT0_DESIGN.md)，明确 KIT/STARTPACK
  的两层范围：声明式 typed 导入优先（P4-KIT-1），运行时随机展开单独立项
  （P4-KIT-2，需产品确认）。
- 记录 `OPTION` / `LOOKUP` / `VALUES` / `EQUIPBUY` / `APPLY` / `QTY` 的
  DS-35E pending 数据、category 混用问题与 `classify(category, tag)` 前置条件。
- `ROADMAP.md` 将 KIT 任务拆为 P4-KIT-0（设计）与 P4-KIT-1（导入）。

### P4-BONUS-1 + P4-EQP-1 第一阶段（已完成）

- typed numeric AC 组件：armor/shield/natural 只进 AC/flat-footed，
  dodge 只进 AC/touch，deflection 等进三字段；修复 Charlie 盾/甲泄漏到 touch。
- `SaveProgression::None` 默认零值，修复无显式 `BONUS:SAVE|BASE.*` 的测试职业
  多算基础豁免。
- flat-footed 保留负 DEX 惩罚；zero-HD class 不再强制 1 HP。
- 公式解析支持 ability modifier（`WIS` 等）与单职业 `CL`，并把 `VAR.*`
  公式在 effect 公式前按依赖多轮求值；修复 3e_MonKee Monk AC 变量/条件。
- broad oracle：BAB/AC/touch/flat 21/21（100%），HP 20/21，save_* 20/21；
  cohort Java-gated 字段扩展；测试 117→119。

### P4-PRE-4 模板 / 手 / 移动谓词（已完成）

- `pcgen-core`：`Predicate` 增加 `TemplateCount` / `HandsAtLeast` /
  `HandsAtMost` / `MovementAtLeast`；`RaceDefinition` 增加 `hands` /
  `movement` / `templates`。
- `pcgen-ir`：`parse_race` 解析 `HANDS:`、`MOVE:Mode,ft`、`TEMPLATE:`；
  `parse_predicate` 支持 `PRETEMPLATE`（容错数据中的尾部 `]`）、`PREHANDSGTEQ`、
  `PREHANDSEQ`、`PREMOVE`。
- `pcgen-engine`：`EffectState` 注入手掌/移动/模板状态（race + draft 模板）；
  `CharacterDraft` 增加 `templates` 输入（`#[serde(default)]`）。
- `P4-PRE-6` 续：补 `PRESIZELT` / `PRESIZELTEQ` / `PRESIZEGT`。
- 真实数据：DS-3E `unsupported_tags` 16→3（四 tag 清零，顺带 lower 1 个 BONUS）；
  DS-35E 152→104。测试 110→114。

### P4-PRE-3 法术类型 / 法术书（已完成）

- `pcgen-core`：新增 `SpellPowerType { Arcane, Divine, Psionic, Unknown }`
  （与 `CastingType` 正交）；`ClassDefinition` 增加 `spell_power` /
  `requires_spellbook`；`Predicate` 增加 `SpellType` / `HasSpellbook` /
  `HasSpellPower` / `CastingTypeIs`。
- `pcgen-ir`：`parse_class` 读 `FACT:SpellType|...` 与 `SPELLBOOK:YES`；
  `parse_predicate` 支持 `PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST`
  （`MEMORIZE=` 与 `TYPE=` 两种形式）。
- `pcgen-engine`：新增阶段 A `derive_spellcasting_prereq()`（只读 class levels +
  静态类定义，不依赖 effects），`EffectState` 注入 spell state；
  `CharacterSnapshot` 增加 `spell_power_types` / `max_spell_level` /
  `has_spellbook`。
- 真实数据：DS-3E `unsupported_tags` 25→16（`PRESPELL*` 清零）；
  DS-35E 276→152（并顺带 lower 使用这些条件的 BONUS）。测试 105→110；
  coverage gate 11→12 pass（新增 DS-35E 阈值/zero-tag 门禁）。

### P4-PRE-6 谓词补齐（部分完成）

- `PRESIZEEQ` → `All(SizeAtLeast, SizeAtMost)`、`PRESIZEGTEQ` → `SizeAtLeast`；
  新增 `normalize_size()` 支持 PCGen 单字母尺寸（S/M/L/H/G/C…）。
- `PRETEXT` 改判 allowlist（纯展示文本）。
- 剩余 `PRETEMPLATE` / `PREMOVE` / `!PRESUBCLASS` / `PRELANG` /
  `PREWEAPONPROF` 需要新运行时状态，按 owner 并入 P4-TPL-1 / P4-EQP-1 /
  P4-RACE-1。

### P4-PRE-5：顶层 `!PRExxx` 取反（已完成）

- `pcgen-ir` 统一识别带 `!` 前缀的谓词 tag：
  - 新增 `pcgen_ir::tags::is_predicate_tag()`，同时匹配 `PRExxx` / `!PRExxx`；
  - `is_known_tag`、lowering 分支、BONUS 条件过滤、Grant 授权键过滤、
    class level 前置串同步改用该判定；
  - `parse_predicate` 对顶层 `!` 递归为 `Predicate::Not(..)`（原来只在
    `PREMULT` 的 `[...]` 组里处理 `!`）。
- DS-3E（回归代理，`srdbasic.pcc`）：`legacy_pending` 1,618→1,489，
  `semantic_completion` 65.4%→67.2%，`unsupported_tags` 保持 25；
  `!PRERACE` / `!PRETYPE` / `!PREABILITY` / `!PRECLASS` / `!PREAGESET` 清零。
- DS-35E（主验收，`_rsrd_basics.pcc`）：421 个 negated tag 离开 legacy，
  其中 399 个 lower 为 `Not(..)`；22 个正向形式尚未支持的
  （`!PRETEMPLATE` / `!PRESUBCLASS` / `!PREMOVE`）转入 unsupported。
- 修复：11 个带 `!PRE` 条件的 BONUS 由“静默去掉条件后生效”改为显式
  unsupported（DS-35E `lowered_effects` 7,768→7,757，属修复而非回归）。
  broad oracle `--check --json` `ok=true`，无 case 数值漂移。
- 测试数 99→104。

### 阶段 0：口径冻结与 coverage 非回归门禁

- 验收数据集固定：DS-3E `srdbasic.pcc`（回归代理）、
  DS-35E `_rsrd_basics.pcc`（主验收）；全量 RSRD 仅作信息报告。
- KIT / COMPANIONMOD / 怪物记录判定为 in scope，计入覆盖分母。
- 新增 `tests/fixtures/coverage/coverage_baseline.json`，冻结两个数据集的
  `entities` / `tag_occurrences` / `lowered_effects` / `semantic_pending` /
  `semantic_completion` 与逐 tag pending 次数。
- `coverage_gate.py` 原先的 histogram 检查只验证"各部分求和 == 总数"，
  数字增长也会通过；现改为**逐 tag 非回归**：任一 tag 的 pending 增加、
  lowered 下降或 completion 下降都会失败，同时允许 legacy ↔ unsupported
  的合法改判。
- 新增 `coverage_gate.py --write-coverage-baseline` 作为唯一的显式重置入口。
- `docs/TESTING.md` 新增验收数据集与基线说明；`PRODUCT_SCOPE.md` 记录
  KIT / COMPANIONMOD / 怪物 in scope；`ROADMAP.md` 新增 P0 阶段并清理旧状态头。
- 门禁 10→11 pass（0 fail / 0 skip）；`semantic_completion`
  DS-3E 67.2% / DS-35E 44.6%。
- `broad_oracle.py --write-baseline` 增加守卫：Java 字段一致率下降或
  `computed → data_missing` 时拒写，必须 `--accept-diff` 才能显式接受；
  正常写入路径已确认与旧 baseline 逐字节一致。
- backlog triage：新增 `docs/TAG_TRIAGE.md`；8 个展示/角色生成/UI tag
  （`AGEDIEROLL`/`BASEAGE`/`MAXAGE`/`HAIR`/`EYES`/`SKINTONE`/`SEX`/`ROLE`）
  改判 allowlist，DS-35E pending 9,367→9,242、completion 44.6%→44.9%；
  82 个 pending tag **全部有 owner**，新建 `P4-KIT-0`（DS-35E pending 的 ~72%）。
  测试 104→105。
- 决策落地：`P4-DEC-1` KIT / COMPANIONMOD in scope、`P4-DEC-2` G3 dropped、
  `P4-DEC-4` G4 superseded、`P4-DEC-3` Pathfinder 1e 正式延后（可复议）。
- 并行产出 `docs/P4_PRE3_SPELL_TYPE_DESIGN.md`（`SpellPowerType` +
  法术书 + 阶段 A/B/C 时序），P4-PRE-3 进入待实现状态。
- 阶段 0 未完成：backlog triage、data marker 恢复、`git init`。

### P4-PRE-1：PREVARNEQ / 残余 PREMULT

- `PREVARNEQ` lower 为 `Not(Variable { min, max })`。
- 新增 `PREPCLEVEL` → `Predicate::CharacterClassLevel { min, max }`；
  `ClassDefinition` 新增 `is_monster`，engine `EffectState` 新增
  `character_class_levels`，等级统计排除 Monster racial HD。
- 真实 3e SRD：`unsupported_tags` 38→35，`PREVARNEQ` / `PREMULT` 清零；
  `semantic_completion` 65.2%→65.3%。
- coverage gate 将 real 3e SRD 的 `unsupported_tags` 阈值 250→35，并新增
  `PREVARNEQ` / `PREMULT == 0` 断言。
- 测试数 92→95。

### P4-PRE-2：PRETOTALAB（已完成）

- 新增 `Predicate::TotalBab` 与 `PRETOTALAB:N` parser；lowering 产物为
  `TotalBab { min: N }`。
- 新增 `pcgen-engine/src/bab.rs`：
  - `compute_bab()` 统一 class progression / formula / gestalt / external
    `COMBAT.BASEAB`，最终 snapshot BAB 与 PRETOTALAB 共用同一路径；
  - `derive_prereq_bab()` 提供不依赖循环的 `BAB_prereq`，并接收
    `EffectState.total_bab`。
- `EffectState` 新增 `total_bab`；`apply_effects` 在 predicate 求值前派生；
  `Engine::evaluate` 改用 `compute_bab()`，`apply_external_modifiers` 不再
  重复添加 BAB。
- 真实 3e SRD：`unsupported_tags` 35→25，`PRETOTALAB` 清零；
  `semantic_completion` 65.3%→65.4%。
- coverage gate：real 3e SRD `unsupported_tags <= 25`，zero-tags 增加
  `PRETOTALAB`。
- 测试数 95→99。
- 时序设计见 `docs/P4_PRE2_PRETOTALAB_DESIGN.md`。

### 计划（Step 2）

- `ROADMAP.md` 的 P4 重写为可验收任务：每项补充目标、指标、门禁、
  完成定义、依赖/风险；按 PRE* 四组、装备/灵能/模板、SPELL effect、
  范围决策拆分。
- 新增 Java 字段一致率表作为排序依据；`ARCHIVE_TEST_MAP.md` 的
  G1–G4 已链接到 `P4-BONUS-1` / `P4-EQP-1` / `P4-DEC-2` / `P4-DEC-4`。

### 门禁效率与口径（Step 1）

- 新增 `scripts/pcgen_cli.py`：门禁脚本构建一次 CLI，并直接执行
  `target/debug/pcgen`，不再对每个 case 启动 `cargo run`。
- `scripts/broad_oracle.py --check --json` 输出结构化摘要，区分
  `computed` / `java_compared` / `known_diverging` / `java_gated` /
  `data_missing`；`--report --json` 同步输出字段一致率 JSON。
- 修复 broad oracle `--check` 中每个 case 被 `evaluate_case` 求值两次的问题。
- `scripts/coverage_gate.py` 改为读取 broad oracle 的 JSON 摘要做分母守卫，
  不再解析人类可读文本。
- `scripts/check.sh` 与 CI full-data 去掉重复的 broad oracle 调用：
  broad oracle 由 coverage gate 调用一次；CLI smoke 直接使用构建好的二进制。
- `docs/TESTING.md` 同步四层门禁、`testdata.lock.json` 升级流程与 JSON 口径说明。

### 架构

- 新增 `pcgen-ir` crate，承载运行时 IR（`RawRecord`、`PatchOperation`、
  typed definitions、`CompiledRulePack`、降低 pass、覆盖度）。
  依赖方向固定为 `pcgen-import → pcgen-ir ← pcgen-engine`。
- `pcgen-import` 只保留 PCGen 文件格式处理（`.pcc` 图、`.lst` tokenize、
  `.pcg` 读写），并 re-export IR 类型以保持调用点兼容。
- `pcgen-engine` 不再依赖 `pcgen-import`（仅 dev-dependency，供 loader+engine
  集成测试）。
- `PcgDraft` / `PcgClassLevel` / `PcgSpell` 数据移入 `pcgen-ir::draft`；
  `.pcg` 解析与写出仍在 `pcgen-import`。
- `PcgDraft → CharacterDraft` 转换从 engine 移到 CLI 适配层。
- 新增 `pcgen_core::EntityCategory` / `ChoiceKind`：
  - `CompiledEntity.category` 由 `String` 改为 `EntityCategory`（serde 仍
    序列化为原 PCGen 字符串，fingerprint 不变）；
  - engine 不再出现 `"RACE"` / `"CLASS_LEVEL"` / `"EQUIPMENT"` / `"PCSTAT"`
    等原始 category 字符串分支，改用 typed match。
- 新增 `scripts/check_architecture.sh` 护栏，并接入 `scripts/check.sh` 与 CI。

### 覆盖度口径

- 新增 `pcgen_core::tagclass`：所有“已识别但未 lower”的 tag 按三位一体分桶
  —— `Allowlisted`（展示/元数据，不计入分母）、`Pending`（legacy 待迁移）、
  `Unsupported`（lowering 拒绝，计入分母）。`TagBucket` 是报告用视图。
- `RuleCoverage` 新增 `legacy_allowlisted_by_tag` / `legacy_pending_by_tag` /
  `unsupported_pending_by_tag` / `unsupported_allowlisted_by_tag`，以及
  `legacy_allowlisted()` / `legacy_pending()` / `unsupported_pending()` /
  `semantic_pending()` / `semantic_completion()`。
- `semantic_completion` 分母 = `lowered + legacy_pending + unsupported_pending`。
  真实 3e SRD 完成率据此为 **46.8%**（此前的 49.5% 未计入 unsupported 337，属高估）。
- `import-report --histogram` 显示 pending legacy / pending unsupported /
  allowlisted 三组。
- coverage gate 断言两套划分都精确：
  `legacy_allowlisted + legacy_pending == legacy_tags`、
  `unsupported_allowlisted + unsupported_pending == unsupported_tags`。
- 文档数字漂移修正（当时值 77）：MIGRATION 标注为历史快照；当前值统一见 `ROADMAP.md`。

### 测试与门禁

- 测试数 72 → 79（当时快照；当前值见 `ROADMAP.md`）（新增 `tagclass`/`TagBucket` 单测与 `tag_split` 集成测试）。

### P3 清账

- broad oracle：cohort 每个 case 声明 `expectation`，新增 `data_missing_limit`
  与 `known_data_gaps`；报告分母固定为 `expectation_summary.computed`，
  data-missing 增长或 expectation 不符会失败。
- 新增 `docs/ARCHIVE_TEST_MAP.md`：archive 114 测试 → 新测试的
  migrated / superseded / dropped 对照表。
- `scripts/snapshot_diff_archive.py`：默认二进制路径指向仓库内 archive，
  升级为 **Java 裁决的三方差分**（archive vs new vs Java csheet），
  接入 `check.sh` 与 CI full-data。new 与 Java 一致而 archive 不一致时为
  `ARCHIVE-BUG`（不算失败）；archive 与 Java 一致而 new 不一致时为
  `REGRESSION`（失败）。
- 三方差分判定 8 个 case：**0 回归 / 0 divergence**；旧 archive 有 2 处自身 bug
  （`3e_BarJack` flat-footed AC、`3e_WizShar` literacy），新引擎在两处均与 Java 一致。
- `ARCHIVE_KNOWN_WRONG` 声明表会与实际观测比对，未再出现的条目打印 `STALE`。

## BONUS 家族 lowering（P4 启动）

- 新增谓词 `Predicate::SizeAtLeast` / `SizeAtMost` / `LegsAtLeast` /
  `RuleEnabled`；`pcgen-core` 导出规范尺寸序 `size_rank/size_at_least/size_at_most`。
- `parse_predicate` 支持 `PREBASESIZELT/LTEQ/EQ`、`PRELEGSGTEQ`、`PRERULE`、
  `PREVARGT/PREVARLT`、`PREAGESET`。
- engine `EffectState` 从种族解析 base size，`PRERULE` 默认开启（可用
  `choices["RULE:<name>"]` 关闭）。
- 真实 3e SRD：`lowered_effects` 2,944 → 3,103；`unsupported` 337 → 177
  （`BONUS` 167 → 8）；`semantic_completion` 46.8% → 49.4%。
- coverage gate 阈值收紧：`lowered_effects >= 3000`、`unsupported_tags <= 250`。
- 行为不变：curated Java 9/9、broad oracle 23 case、三方差分 0 回归。

## PRE 家族 lowering（P4 续）

- 新增谓词 `Predicate::AtLeast` / `RaceType` / `AgeCategory`。
- `parse_premult`：括号感知解析 `PREMULT:N,[..],[..]`，支持嵌套与 `!` 取反；
  `N == 组数` 时降为 `All`。
- `PRERACETYPE` → `RaceType`（engine 从种族 `type_tags` 求值）；
  `PREAGESET` → `AgeCategory`（未知年龄不匹配，保守，取代此前的 `Always`）。
- 真实 3e SRD：`lowered_effects` 3,103 → 3,107；`unsupported` 177 → 38
  （`PREMULT` 113 → 1）；`semantic_completion` 49.4% → 50.5%。
- 剩余 38 个 unsupported 全部是**建模缺口**（需要总 BAB / 法术类型 / 模板集合 /
  手数等运行时状态），已在 `docs/TAG_WORKLIST.md` 列表化。
- 行为不变：curated Java 9/9、broad oracle 23 case、三方差分 0 回归。

## Ability pool（P4 续）

- `CompiledEntity` 新增 `ability_category: Option<String>`；`ABILITY` /
  `ABILITYCATEGORY` 记录的 `CATEGORY:` 被捕获为 ability pool，不再计入 legacy。
- engine `find_entity` 解析 `Grant` 时按 pool 优先匹配（同 key 不同 pool 不会错配）。
- 真实 3e SRD：`legacy_tags` 4,289 → 3,348（`CATEGORY` 941 → 0）；
  `semantic_completion` 50.5% → 59.6%。
- 行为不变：curated Java 9/9、broad oracle、三方差分 0 回归。

## broad oracle 分母做实（P4 续）

- 修正 5 个 data-missing case 的 **PCC 映射**（此前映射反了）：
  `Test Nothing` / `Test46b` / `Test51H` / `Add SpellCaster` 在 PTS，
  `Giant` / `Monstrous humanoid` 在 RSRD。
- `JimDop` / `CloudGiant` 因缺 **monster HD progression** 无法算对，新增
  cohort expectation `computed_diverging`：必须能计算，但不要求与 Java 一致，
  且不计入一致率分母（数值仍冻结）。
- broad oracle：`18 → 23 compared`，**data-missing 0**；新增
  `must_compute` 与 coverage gate 的"分母不得缩小"守卫。
- 一致率（分母 21）：`final_stats/initiative 100%`、`save_fortitude 85.7%`、
  `armor_class 81.0%`、`bab 61.9%` 等，如实反映剩余差异。

## 装备元数据 + monster HD 修复（P4 续）

- **monster class BAB 公式修复**：`classlevel()`（空括号）未被正则匹配，
  导致 Giant / Monstrous humanoid 等怪物职业 BAB 全为 0。修正后
  `JimDop` BAB 2→6、`CloudGiant` 0→12，与 Java 完全一致。
- `class_progression_is_populated`：monster class 的空 progression 不再
  屏蔽其 formula BONUS。
- **装备元数据 typed 化**：`EquipmentModifierDefinition` 增加 `plus` /
  `item_types` / `replaces`；`EquipmentDefinition` 增加 `damage` /
  `crit_mult` / `crit_range` / `wield`。EQUIPMOD/EQUIPMENT 对应 tag 不再计入 legacy。
- 真实 3e SRD：`legacy_tags` 3,348 → 2,902；`semantic_completion` 59.6% → 65.2%。
- 行为不变（monster BAB 修复除外，属修正）：curated Java 9/9、broad oracle、三方差分 0 回归。

## 说明

更早的 Sprint 历史见 `MIGRATION.md`；当前状态见 `ROADMAP.md`。
