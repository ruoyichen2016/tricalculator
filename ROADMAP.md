# PCGen-Rust Roadmap

> 本文档是路线与状态的**唯一事实源**。Sprint 历史见 `MIGRATION.md`，
> 测试/门禁与数据版本见 `docs/TESTING.md`，待迁移 tag 见 `docs/TAG_WORKLIST.md`。
> 数字来自固定数据 `scripts/testdata.lock.json`
> （ref `f91bd03f2dbe1145d5526b52aa3e48fcd0f2fc8d`）。

## 当前状态（最近验证：固定数据 + 全量门禁）

- 工作区：7 crate（core / formula / ir / import / engine / output / cli）。
- 测试：`cargo test --workspace` **197 passed**。
- DS-3E（回归代理，`srdbasic.pcc`）：3,779 entities，23,723 tag occurrences，
  lowered 3,267；legacy pending 0；unsupported 0；
  **semantic completion 100.0%**。
- DS-35E（主验收，`_rsrd_basics.pcc`）：15,741 entities，
  lowered 8,196；legacy pending 0；unsupported pending 0
  （仅 2 个 `PRETEXT` allowlisted）；**semantic completion 100.0%**。
- DS-PSI（灵能验收，`_rsrd_psionics.pcc`）：1,570 entities，
  lowered 1,195；legacy pending 0；unsupported pending 0；
  `psionic_effects` 50；**semantic completion 100.0%**。
- P0 口径清偿：`CHOOSE` 已经 lowering，不再重复计入 legacy pending；
  `XPCOST` 按 P4-DEC-2 移入 out-of-scope allowlist；三个数据集的
  `unsupported_pending` 均被门禁显式要求为 0。
- 门禁 `scripts/coverage_gate.py`：**14 pass / 0 fail / 0 skip**（严格模式，含三数据集 coverage 基线与 DS-35E 阈值）。
- broad per-case oracle：23 cohort（**23 computed = 23 java-compared +
  0 known-diverging**，0 data-missing），23 个 java-gated case；
  `final_stats` / `bab` / `hit_points` / `initiative` / `armor_class` /
  `touch_ac` / `flat_footed_ac` / `save_*` 全部 **23/23（100%）**。
- curated Java differential：9/9。

## P0：口径冻结（阶段 0，已完成）

- [x] 验收数据集固定：DS-3E `srdbasic.pcc`（回归代理）、
      DS-35E `_rsrd_basics.pcc`（主验收）；全量 RSRD 仅作信息报告
- [x] KIT / COMPANIONMOD / 怪物记录判定为 in scope，计入覆盖分母
- [x] 新增 `tests/fixtures/coverage/coverage_baseline.json`，把原先只做
      "自洽校验"的 histogram 门禁升级为**双数据集、逐 tag 非回归**门禁
- [x] 新增 `coverage_gate.py --write-coverage-baseline` 显式更新入口
- [x] 文档同步：`docs/TESTING.md` 第 7 节、`PRODUCT_SCOPE.md`
- [x] backlog triage：新增 [`docs/TAG_TRIAGE.md`](docs/TAG_TRIAGE.md)，
      8 个展示/角色生成 tag 改判 allowlist；82 个 pending tag **全部有 owner**
      （新增 `P4-KIT-0` 等，见 P4-F）
- [x] `P4-DEC-1`（KIT/COMPANIONMOD in scope；SIZE 数值保留）、
      `P4-DEC-2`（G3 dropped）、`P4-DEC-4`（G4 superseded）结论写入文档
- [x] `P4-DEC-3` 结论：**正式延后（可复议）**。纳入前置条件：定义数据来源、
      profile 范围、首个 acceptance case 与 profile smoke 门禁；满足后再单独评估
- [x] 工程硬化：恢复 `.pcgen-rust-testdata` data marker；`git init` +
      初始提交 + `.gitignore`；broad oracle `--write-baseline` Java 一致率
      下降时拒写（需 `--accept-diff` 才可显式接受）
- [x] 清理已完成阶段的状态头（P1/P2/P3）

### P0.4 阶段 3：能力池 / 选择语义闭环（已完成）

- [x] `AbilityDefinition` / `AbilityCategoryDefinition` typed 模型。
- [x] `ABILITYLIST` 成员关系进入 `CharacterSnapshot.ability_lists`。
- [x] `POOL` 池点数从变量解析进入 `CharacterSnapshot.ability_pools`。
- [x] `STACK` / `MULT` 进入 repeatable ability metadata。
- [x] `CHOICE` 进入 class subclass choice metadata。
- [x] `CHOOSE` 优先从 `draft.choices` 解析；无法确定的动态选择进入
      `unresolved_choices` 并输出 diagnostics，不再静默取 first/0。
- [x] broad oracle baseline 新增 choice / unresolved / ability pool / list /
      repeatable ability 状态，覆盖现有 23-case cohort。
- [x] 选择类 pending 清零：DS-3E 1（`ATTACKCYCLE`）、DS-35E 2
      （`MOVECLONE` / `REACH`）、DS-PSI 0；completion 均为 100.0%。

### P0.5 P4 全量关闭：residual / language / subclass / spell / decisions（已完成）

- [x] residual pending 清零：`ATTACKCYCLE` / `MOVECLONE` / `REACH` typed
      消费并进入 snapshot。
- [x] language state：`LANGBONUS` / `AUTO:LANG` 进入 `languages`，
      `PRELANG` 从真实语言集合求值。
- [x] subclass state：`AdvancementLane.subclass` + `PRESUBCLASS` 真实求值。
- [x] ability pool usage：`ability_pool_usage` 输出 pool / spent / remaining /
      selected。
- [x] bounded numeric tails：`ADD:SPELLCASTER|Arcane/Divine` PRC caster
      stacking；monster base HD 平均 HP。
- [x] 范围决策：KIT runtime deferred、Pathfinder deferred；见
      [`docs/P4_DECISIONS.md`](docs/P4_DECISIONS.md)。
- [x] 三数据集 pending 全部为 0，completion 100.0%；P4 closure matrix 已写入
      本文件顶部。

### P0.3 阶段 2：剩余装备 / 法术 / 灵能 pending 清零（已完成）

- [x] P4-EQP-2：`ALTCRITMULT` / `ALTCRITRANGE` / `ALTDAMAGE` /
      `ALTEQMOD` / `ALTTYPE` / `ARMORTYPE` / `BASEQTY` / `CHANGEPROF` /
      `WEAPONBONUS` 全部 typed 消费；alternate weapon profile 与
      `PREWEAPONPROF` 状态接入 engine。
- [x] P4-SPELL-2：`ADDDOMAINS` / `PROHIBITCOST` / `DONOTADD` /
      Spell `VARIANTS` 全部 typed 消费；条件 `ADDDOMAINS` 进入
      `domain_spells`，`PROHIBITCOST` 进入 snapshot `prohibited_costs`。
- [x] P4-PSI-2：Wilder 第三个灵能职业 golden、`powers_known` 进入
      snapshot、PRC `ADD:SPELLCASTER|Psionic` 叠加到基础 manifesting level。
- [x] 三数据集目标 tag pending 均为 0；baseline 重冻结为
      DS-3E 35 / 98.9%，DS-35E 100 / 98.8%，DS-PSI 11 / 99.1%。

### P0.2 阶段 1：typed-but-unconsumed 运行时闭环（已完成）

- [x] P4-CLASS-2：`SKILLMULT` 进入首级技能点，`LEVELSPERFEAT` 进入
      `ClassProgression.feat_slots()` / snapshot `feat_slots`，`HITDIE`
      进入无显式 HP 输入时的平均 HP，`ADDLEVEL` 进入 effective class levels，
      `EXCLASS` / `EXCHANGELEVEL` 进入 progression 与 snapshot
      `class_interactions`；新增 `class_progression.rs` golden。
- [x] P4-STAT-2：`DEFINESTAT:MINVALUE` 进入属性下限，`STATMOD` 进入
      派生属性修正，`SIZENUM` 进入 `size_rank` / size modifier；新增
      `stats_and_size.rs`。
- [x] P4-FAITH-2：`VALIDFORDEITY` / `VALIDFORFOLLOWER` 进入信仰诊断，
      并覆盖 deity / alignment 两侧；扩展 `faith_rules.rs`。
- [x] P4-COMP-1：CompanionMod 行级 `PRERACE` 匹配、`HD` / `SR` /
      `ABILITY` / creature traits / `BONUS:COMBAT|AC` / `BONUS:STAT`
      进入 `CompanionSnapshot`；扩展 `creature_traits.rs`。
- [x] 真实数据指标与 broad oracle 不变：DS-3E 75 / 97.8%，DS-35E 192 /
      97.7%，DS-PSI 35 / 97.2%；broad 23/23，archive 0 regression。

### P0.1 质量清偿 / 口径修正（已完成）

- [x] `CHOOSE` 不再同时计入 `lowered_effects` 与 `legacy_pending`；新增 lowering
      回归测试，pending 只保留真正未 lower 的 tag。
- [x] `XPCOST` 依据 P4-DEC-2 移入 out-of-scope allowlist，不再计入
      `semantic_pending`。
- [x] CompanionMod `FOLLOWER:` 行改为按 id 分组保存，真实 RSRD 108 行不再
      被覆盖；带 `PRERACE`/`PRE*` 的条件行先保留、不在 runtime 盲目叠加。
- [x] coverage gate 收紧：三个数据集 unsupported pending 必须为 0；
      DS-35E 的 unsupported allowlisted 精确允许为 2（`PRETEXT`）。
- [x] `TAG_WORKLIST` 按 P0 后的 DS-3E 重新生成；`TAG_TRIAGE` 标记为历史快照。

## P1：可追溯性与门禁闭环（已完成）

- [x] 固定测试数据 lock（`scripts/testdata.lock.json`），升级只改一个文件
- [x] `fetch_testdata.sh` / `check_testdata.sh` 读 lock，含 blob hash 校验
- [x] CI 拆 `no-data` / `full-data`，gate 不再把 SKIP 当 PASS
- [x] broad oracle：cohort + baseline + per-case 门禁
- [x] 规则包 fingerprint（`pcgen-core::fingerprint`）+ snapshot/report provenance
- [x] `unsupported_by_tag` / `legacy_by_tag` 直方图 + `docs/TAG_WORKLIST.md`
- [x] 归档 README/ROADMAP/MIGRATION 进度数字（MIGRATION 明确标注为历史快照）

## P2：架构收口（已完成）

- [x] 切断 `pcgen-engine → pcgen-import`（新增 `pcgen-ir`，含 `CompiledRulePack` + 降低 pass）
- [x] `PcgDraft → CharacterDraft` 转换移出 engine（改到 CLI 适配层）
- [x] `PcgDraft` 数据类型移入 `pcgen-ir`（`.pcg` 解析仍在 import）
- [x] engine 内字符串 tag 语义 typed 化（`EntityCategory` / `ChoiceKind`）
- [x] `legacy` 分类：allowlist（展示字段）vs pending（语义字段），并给出 `semantic_completion`

## P3：清账（已完成）

- [x] 文档数字口径收口（MIGRATION 标注为历史快照；当前值以本文件顶部为准）
- [x] `unsupported_by_tag` 接入 tagclass：allowlisted / pending / unsupported 三位一体，
      `semantic_completion` 分母改为 `lowered + legacy_pending + unsupported_pending`
- [x] archive 114 测试 → 当前测试对照表（见 `docs/ARCHIVE_TEST_MAP.md`）
- [x] broad oracle 固定“应算集合”（cohort `expectation` + `data_missing_limit`）
- [x] archive differential 接入 full-data CI，并升级为 Java 裁决的三方差分
      （ARCHIVE-BUG / REGRESSION / DIVERGENCE 分类）
- [x] 门禁口径与效率：broad oracle `--check --json` 区分 computed /
      java-compared / known-diverging；coverage gate 读结构化摘要；
      check.sh / CI 去重；`scripts/pcgen_cli.py` 直接调用构建好的 CLI

### P3 派生发现（archive differential 暴露，已判定）

archive differential 改为 **三方差分**（archive vs new vs Java），由 Java 裁决：

- [x] `3e_BarJack` flat_footed_ac：archive=16 / new=19 / **java=19** → 新引擎正确，
      旧 archive 未处理 barbarian uncanny dodge
- [x] `3e_WizShar` skills.Literacy：archive=3 / new=1 / **java=1** → 新引擎正确，
      旧 archive 错误叠加了 synergy
- 结论：8 个 case **无新引擎回归**；archive 作为 oracle 在 2 处自身有 bug。

## P4：按用户价值迁移语义（已完成；deferred 项已标注）

> P4 已重写为可验收任务。每项必须回答：目标是什么、用什么指标判断、
> 由哪个门禁守住、什么条件下算完成、依赖/风险是什么。
> `docs/ARCHIVE_TEST_MAP.md` 的 G1–G4 已链接到对应任务。
>
> 已完成基线：`CATEGORY` ability pool（941→0）、`PREMULT` 主体（113→1）、
> BONUS 家族主体（167→4）、5 个 data-missing case 修复（0 data-missing）。

### P4 closure matrix

| ID | 最终状态 | 说明 |
|---|---|---|
| P4-PRE-1 | completed | `PREVARNEQ` / `PREMULT` |
| P4-PRE-2 | completed | `PRETOTALAB` 两阶段求值 |
| P4-PRE-3 | completed | `PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST` |
| P4-PRE-4 | completed | `PRETEMPLATE` / `PREHANDS*` / `PREMOVE` |
| P4-PRE-5 | completed | 顶层 `!PRExxx` |
| P4-PRE-6 | completed | size 谓词、`PRETEXT` allowlist；`PRELANG` / `!PRESUBCLASS` 运行时状态已落地 |
| P4-ENG-1 | completed | class/level 激活与 modifier 归一化 |
| P4-BONUS-1 | completed | typed bonus / stacking |
| P4-EQP-1 / P4-EQP-2 | completed | equipment slots、alternate profile、proficiency |
| P4-PSI-1 / P4-PSI-2 | completed | 灵能闭环、Wilder、`powers_known`、PRC stacking |
| P4-TPL-1 | completed | template 激活与 creature traits |
| P4-SPELL-1 / P4-SPELL-2 | completed | spell effect model、domain/prohibited/cost、arcane/divine PRC caster stacking |
| P4-DEC-1 | closed | KIT runtime deferred / CompanionMod completed / SIZE numeric completed |
| P4-DEC-2 | dropped | G3 XP / point buy out-of-scope |
| P4-DEC-3 | deferred | Pathfinder 1e 明确在 P4 外，见 `docs/P4_DECISIONS.md` |
| P4-DEC-4 | superseded | G4 由 Rule IR 取代 |
| P4-DATA-1 | completed | monster HD / Java parity |
| P4-KIT-0 | completed | KIT spike |
| P4-KIT-1 | completed | declarative typed import |
| P4-KIT-2 | deferred | runtime random expansion 不在 P4，见 `docs/P4_DECISIONS.md` |
| P4-TEMP-1 | completed | active-state |
| P4-TEMP-2 | deferred | TEMPVALUE UI / `%CHOICE` 交互扩展，不在 P4 |
| P4-RACE-1 / P4-RACE-2 | completed | `REACH` / `MOVECLONE` / `ATTACKCYCLE`、creature traits、monster HD |
| P4-CLASS-1 / P4-CLASS-2 | completed | 职业 / 进阶 typed + runtime |
| P4-STAT-1 / P4-STAT-2 | completed | 属性 / size typed + runtime |
| P4-FAITH | completed | deity / domain / valid flags |
| P4-ABILITY-1 | completed | ability pool / choice / stack / mult |
| P4-COMP-1 | completed | companion runtime |

> `deferred` 项不阻塞 P4 关闭；若重新纳入，必须重新开 task、定义 acceptance
> case 和门禁并更新本矩阵。

### P5-1 PCG choice / subclass ingestion（已完成）

- `.pcg` 解析 `CLASS:...+SUBCLASS:...` 进入 `PcgDraft.class_subclasses`；
- `.pcg` 解析 `ABILITY:...|KEY:...|APPLIEDTO:...` 进入 `PcgDraft.choices`；
- CLI `draft_from_pcg()` 将 choices / class_subclasses 传入 `CharacterDraft`；
- engine `apply_choose` 支持按 source entity 上下文查找 `draft.choices`；
- `PRESUBCLASS` 支持 `class_subclasses`；
- `TEMPLATESAPPLIED` 内嵌 `APPLIEDTO` 进入 choices；`CLASS|ALL` / `CLASS|PC` 动态 class list 解析；curated cohort 已无 unresolved choices。

### P5-14 KIT runtime 决策启动 + Pathfinder 立项评估（已完成）

- `docs/P5_KIT_RUNTIME_DESIGN.md`：冻结 KIT runtime 启动条件
  （确定性 RNG、语义规范、Java 对照、独立门禁）；实现仍 deferred。
- `docs/P5_PATHFINDER_ASSESSMENT.md`：评估完成，结论为
  **deferred / 不立项**，并列出重新评估条件。
- 两项均不阻塞当前 3e/3.5e 交付。

### P5-13 Web GUI 导出与提示（已完成）

- `POST /api/export`：draft overrides 后导出可回写 `.pcg`，返回
  `unresolved_choices` 与 `ability_pool_usage`。
- 页面新增：Export `.pcg` 按钮、导出文本面板、unresolved choice 红色提示、
  pool 剩余额度进度条（超支红色）。
- web smoke 测试 6→7。

### P5-12 Web GUI 增强（已完成）

- `/api/curated`：从 broad cohort 读取 curated case，支持
  `--cohort-root` 与 `--data-root` 分离。
- `POST /api/recalculate` 支持 `overrides`：
  `choices` / `abilities` / `active_states` / `class_subclasses` /
  `deity` / `domains`，用于 choice 覆盖与 ability pool 编辑。
- 页面新增 curated 下拉、choice 覆盖表、ability 选择表、active state 输入。
- web smoke 测试 4→6。

### P5-11 本地 Web GUI（已完成）

- 新增 `pcgen-web` crate（axum + tokio）：`/`、`/api/health`、
  `/api/recalculate`。
- 页面展示 snapshot JSON、choices / unresolved_choices /
  ability_pool_usage、diagnostics 与 HTML 角色卡。
- `pcgen-web` 复用 `pcgen-import` + `pcgen-engine` + `pcgen-output`，
  不侵入 engine / output 架构。
- 新增 `crates/pcgen-web/tests/web_smoke.rs` 并接入 CI no-data 与
  `scripts/check.sh`。

### P5-8 / P5-9 P5-10 收尾（已完成）

- **P5-8**：spell numeric v2：`applied_spell_effects` 折叠进 AC / saves /
  stats / skills / attack 数值；`prepare/memorized` 与 buff allowlist
  双重门控，避免误把已知法术当激活 buff。
- **P5-9**：curated PCG 的 choice / pool / domain 状态进入 broad
  `choice_state` baseline；真实数据 Java 字段保持 23/23。
- **P5-10**：`docs/P4_DECISIONS.md` 明确 KIT runtime / TEMPVALUE UI /
  Pathfinder 均为 deferred；不启动，不阻塞当前核心路线。

### P5-6 / P5-7 真实角色正确性基线与 spell numeric（已完成）

- **P5-6**：`choice_state` baseline 扩展为
  `choices` / `choice_bindings` / `ability_pools` / `ability_pool_usage` /
  `ability_lists` / `repeatable_abilities` / `domain_spells` /
  `deity_domains`；curated cohort 的 choice/pool/domain 状态可断言。
- **P5-7**：spell numeric v1：已准备法术与领域法术的
  `BONUS:COMBAT|AC` / `BONUS:SAVE` / `BONUS:STAT` / `BONUS:SKILL`
  进入 `CharacterSnapshot.applied_spell_effects`。
- 三数据集 pending 保持 0，completion 100.0%；broad / archive 全绿。

### P5-5 PCG 选择元数据补强（已完成）

- `CHOSENTEMPLATE:[NAME:...]` 嵌套选择：chosen template 名称与 choice 均被保留。
- `DEITYDOMAINS:[DOMAIN:...]` 与选定 `DOMAIN:` 分离：catalog 进入
  `deity_domains`，选定 domain 仍进入 `domains`。
- 信仰校验使用 `.pcg` catalog 验证选定 domain。
- `CharacterSnapshot.choice_bindings` / `deity_domains` 可断言。
- broad cohort 中 cleric / specialist / template-choice case 的
  choice_state 已更新并冻结。

### P5-4 Choice Binding 与能力池计数（已完成）

- `EffectState.choice_bindings`：choice 绑定到发起 `CHOOSE` 的 source entity。
- `%CHOICE%` / `%LIST%` 优先使用 source-bound choice，不再取全局第一个。
- Formula normalization 同样使用 source-bound choice。
- `ability_pool_usage.selection_counts` 记录重复选择次数；
  `spent` 变为真实计数总和。
- 重复选择但没有 `MULT` 的能力写入 diagnostics。

### P5-3 Pathfinder 试运行（已完成，仍 deferred）

- Pathfinder Core Rulebook 可完成语法导入与基础 recalculate；
- 修复 Windows 路径、KEY-only support PCC、`|PRECAMPAIGN` 条件路径；
- 结果级不可用：Pathfinder 专属模型、公式与 archetype/traits 仍缺失；
- 正式结论仍为 deferred，详见 [`docs/P5_PATHFINDER_TRIAL.md`](docs/P5_PATHFINDER_TRIAL.md)。

### P5-2 PCG 选择元数据与能力池诊断（已完成）

- `CLASSABILITIESLEVEL` 内嵌 `ADD:[...|CHOICE:...]` 解析到 `PcgDraft.choices`。
- `SPECIALTIES:[SPECIALTY:...]` 解析到 `class_subclasses`。
- `.pcg DEITY` / `DOMAIN` 进入 `CharacterDraft.deity` / `domains`，并生成
  non-gestalt `DivineTrack`，进入 `faith_decisions` 与 `domain_spells`。
- ability pool overspend 回写 diagnostics：`ability pool ... overspent`；
  `ability_pool_usage.selected` 保留实际选中能力。

### P4 排序依据：Java 字段一致率

数据来自 `python3 scripts/broad_oracle.py --report --json`。
口径：cohort 中 21 个 Java-compared case；2 个 `computed_diverging`
不计入一致率分母。

| Java 字段 | 一致率 | 主要候选 P4 项 |
|---|---:|---|
| `bab` | **100.0%（23/23）** | ✅ P4-ENG-1 |
| `armor_class` | **100.0%（23/23）** | ✅ P4-BONUS-1 / P4-EQP-1 第一阶段 |
| `touch_ac` | **100.0%（23/23）** | ✅ typed AC 组件 |
| `flat_footed_ac` | **100.0%（23/23）** | ✅ typed AC + 负 DEX 惩罚 |
| `hit_points` | **100.0%（23/23）** | ✅ per-level HP minimum + template CON |
| `save_fortitude` | **100.0%（23/23）** | ✅ Halfling racial trait / template CON |
| `save_reflex` | **100.0%（23/23）** | ✅ Halfling racial trait |
| `save_will` | **100.0%（23/23）** | ✅ Halfling racial trait |
| `final_stats` | **100.0%（23/23）** | ✅ template stat adjustments |
| `initiative` | **100.0%（23/23）** | — |

排序原则：

1. 先做能直接清 unsupported / legacy、且不依赖新运行时状态的低风险项；
2. 再做能提升低一致率字段、且能由 Java csheet 精确裁决的重任务；
3. Java 一致率只决定重任务之间的先后，不得通过放宽 baseline 换取通过。

### P4-A PRE* 组

unsupported tag 的清零路径（起点为真实 3e SRD 的 `unsupported_tags=38`）。
P4-PRE-5 额外清理 legacy_pending（不改变 unsupported 路径）：

| 阶段 | 清零 tag（次数） | 阶段后 `unsupported_tags` |
|---|---|---:|
| 起点 | — | 38 |
| P4-PRE-1 | `PREVARNEQ` 2 + `PREMULT` 1 | 35 |
| P4-PRE-2 | `PRETOTALAB` 10 | 25 |
| P4-PRE-3 | `PRESPELLTYPE` 7 + `PRESPELLBOOK` 2 | 16 |
| P4-PRE-4 | `PRETEMPLATE` 5 + `PREHANDSGTEQ` 3 + `PREHANDSEQ` 2 + `PREMOVE` 2 | 3 |
| P4-EQP-1 / P4-BONUS-1 | 残余 `BONUS` 3（装备类条件） | 0 |

#### P4-PRE-1 纯表达式谓词（PREVARNEQ / 残余 PREMULT）

- **状态**：已完成（真实 3e SRD `unsupported_tags` 38→35；`PREVARNEQ` / `PREMULT` 均为 0；测试 92→95）。
- **目标**：把不依赖新运行时状态的残余谓词全部 lower 为现有 `Predicate`。
- **指标**：`unsupported_pending_by_tag` 中 `PREVARNEQ=0`、`PREMULT=0`；
  `unsupported_tags <= 35`。
- **门禁**：`pcgen-ir/tests/predicate_lowering.rs` 新增用例；
  `coverage_gate.py` 断言这两个 tag 为 0；broad oracle baseline 无变化；
  curated Java 9/9。
- **完成定义**：真实 3e SRD 报告为 0；`docs/TAG_WORKLIST.md` 更新；
  残余 PREMULT 的解析边界写入文档。
- **依赖/风险**：无；残余 PREMULT 可能是解析边界，需要先确认样本。

#### P4-PRE-2 派生数值谓词（PRETOTALAB）

- **状态**：已完成（真实 3e SRD `unsupported_tags` 35→25，`PRETOTALAB=0`；测试 95→99；时序设计见 [`docs/P4_PRE2_PRETOTALAB_DESIGN.md`](docs/P4_PRE2_PRETOTALAB_DESIGN.md)）。
- **目标**：定义并实现 `PRETOTALAB` 的求值时序。total BAB =
  progression BAB + 已生效 BAB modifier，且不引入循环求值。
- **指标**：`PRETOTALAB=0`；`unsupported_tags <= 25`；
  新增测试覆盖 `PRETOTALAB:<n>` 边界与 BAB modifier 叠加。
- **门禁**：engine/predicate 单元测试；broad oracle baseline 无变化；
  `coverage_gate.py` 把 real 3e SRD 的 `unsupported_tags` 收紧到 <= 25，并断言 `PRETOTALAB == 0`；能找到合适 Java case 时加入 curated。
- **完成定义**：`ARCHITECTURE.md` 记录两阶段顺序；10 处 PRETOTALAB lower；
  无 BAB 循环诊断。
- **依赖/风险**：谓词求值可能早于 Effect 应用，total BAB 尚未确定；
  这是本项最大设计风险，先写设计再实现。

#### P4-PRE-3 法术类型 / 法术书谓词（PRESPELLTYPE / PRESPELLBOOK / PRESPELLCAST）

- **状态**：**已完成**（设计见 [`docs/P4_PRE3_SPELL_TYPE_DESIGN.md`](docs/P4_PRE3_SPELL_TYPE_DESIGN.md)）。
  DS-3E `unsupported_tags` 25→16、`PRESPELLTYPE`/`PRESPELLBOOK`/`PRESPELLCAST` 全 0；
  DS-35E 276→152（`PRESPELLTYPE`/`PRESPELLCAST`/`PRESIZEEQ`/`PRESIZEGTEQ` 清零，
  并顺带 lower 掉一批使用这些条件的 BONUS）。
- **目标**：建立独立于 `CastingType` 的 `SpellPowerType`（arcane / divine / psionic）
  与法术书/施法模式状态，lower `PRESPELLTYPE`、`PRESPELLBOOK`、`PRESPELLCAST`；
  `PRESPELLSCHOOL` 语义待确认放二期。
- **指标**：`PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST` 在 DS-3E / DS-35E
  均为 0；DS-3E `unsupported_tags <= 16`；新增 typed spell state 字段与测试。
- **门禁**：curated caster cases 的 spell_dcs / prepared_spells /
  caster_level 保持通过；broad baseline 无变化；coverage gate 双数据集。
- **完成定义**：`SpellPowerType` 进入 `pcgen-ir` + `CharacterSnapshot`
  （`CharacterDraft` 不新增输入，法术书是派生值）；三 tag 清零；文档更新。
- **依赖/风险**：设计已冻结阶段 A/B/C 时序；`PRESPELLSCHOOL` 语义待确认；
  effect 动态改变施法者等级 v1 不做（记 open issue）。与 P4-SPELL-1 共用模型。

#### P4-PRE-4 模板 / 手 / 移动谓词（PRETEMPLATE / PREHANDS* / PREMOVE）

- **状态**：**已完成**。新增 `TemplateCount` / `HandsAtLeast` / `HandsAtMost` /
  `MovementAtLeast`，state 由 race（`HANDS`/`MOVE`/`TEMPLATE`）与 draft 模板派生；
  DS-3E unsupported 16→3（四 tag 清零并顺带 lower 1 个 BONUS）。
- **目标**：为模板集合、手数、移动模式建立运行时状态；
  lower `PRETEMPLATE`、`PREHANDSGTEQ`、`PREHANDSEQ`、`PREMOVE`。
- **指标**：这四个 tag 均为 0；`unsupported_tags <= 3`（只剩装备类 BONUS 残余）。
- **门禁**：单元测试；broad baseline 无变化；`coverage_gate.py` 断言 tags。
- **完成定义**：状态进入 engine；四 tag 清零；模板部分与 P4-TPL-1 合并验收，
  手/移动部分与 P4-EQP-1 合并验收。
- **依赖/风险**：需要定义 hand count 的来源（race / equipment / effects）
  和 movement modes 的枚举。

#### P4-PRE-5 顶层 `!PRExxx` 取反（已完成）

- **状态**：已完成。DS-3E（回归代理）`legacy_pending` 1,618→1,489，
  `semantic_completion` 65.4%→67.2%，`unsupported_tags` 保持 25；
  测试 99→104。
- **目标**：把 PCGen 的 `!PRExxx` 与 `PRExxx` 统一建模为 `Not(Predicate)`，
  不再作为未知 tag 静默丢弃。
- **指标**：DS-3E 中 `!PRERACE` / `!PRETYPE` / `!PREABILITY` / `!PRECLASS` /
  `!PREAGESET` 清零；`unsupported_tags` 不增加；DS-35E（主验收）421 个
  negated tag 全部离开 legacy（399 个 lower 为 `Not(..)`）。
- **门禁**：`pcgen-ir/tests/predicate_lowering.rs` 新增顶层 negated、BONUS
  条件与 unsupported-inner 三类用例；broad oracle `--check --json` `ok=true`；
  baseline 无变化。
- **完成定义**：`tags::is_predicate_tag`、lowering 分支、`parse_predicate`
  三处一致识别 `!PRE`；BONUS 条件、Grant 授权键过滤、class level 前置串同步。
- **依赖/风险**：正向形式尚未支持的 `!PRETEMPLATE` / `!PRESUBCLASS` /
  `!PREMOVE` 会转入 unsupported（DS-35E +22），属正确的显式保留；
  11 个带 `!PRE` 条件的 BONUS 由“静默去掉条件后生效”改为 unsupported
  （DS-35E `lowered_effects` 7,768→7,757），属修复而非回归。

### P4-ENG-1 类/等级实体激活与 modifier target 归一化（已完成）

- **状态**：已完成。修复 stable `EntityId` key（`charlie_class`）与 draft
  class id（`Charlie Class`）不一致导致的 class/class-level 实体不激活；
  `AttackProgression` 默认改为 `None`，无显式 `BONUS:COMBAT|BASEAB` 的职业
  BAB 为 0；补 `HP.CURRENTMAX`、`TL` 公式与 progression lookup 归一化，
  并避免 raw BASEAB / BASE.SAVE 与 typed progression 重复计算。
- **指标**：broad oracle `bab` 13/21→**21/21（100%）**；
  `armor_class` 17→20、`touch_ac` 17→19、`flat_footed_ac` 16→19、
  `hit_points` 15→17；saves 不下降；`java_gated` 18→21（新增 BAB）。
- **门禁**：`pcgen-engine/tests/effects.rs` 新增无 BASEAB 职业、带空格类 key、
  `HP.CURRENTMAX`、BASEAB/BASE.SAVE 不重复计数用例；
  `broad_oracle.py --check --json` 必须 `ok=true`；`coverage_gate.py` 严格模式；
  `bash scripts/check.sh` 全绿。
- **完成定义**：class/class-level effects 只应用一次；BAB 与 Java 一致；
  broad baseline 已按 Java 收敛重生成；`ROADMAP` / `CHANGELOG` 更新。
- **依赖/风险**：Charlie `touch_ac` 仍需要 P4-BONUS-1 / P4-EQP-1 的 typed
  AC 组件；HP 的 1-vs-0 边界与 monster HD 归 P4-DATA-1 / P4-BONUS-1。

### P4-B 装备 / 灵能 / 模板语义

> **合并验收说明**：`P4-BONUS-1` 与 `P4-EQP-1` 共用 typed modifier /
> bonus stack / AC 组件模型，后续按一个联合门禁验收；保留两个 ID 仅用于
> `ARCHIVE_TEST_MAP.md` G1/G2 追溯。

#### P4-BONUS-1 + P4-EQP-1 合并验收：BONUS 残余 / G1 stacking / G2 装备

- **状态**：残余 `BONUS` unsupported 已清零；AC / saves / attack / skill
  已接入统一 `TypedBonus` 管线（bonus type / stack key / target 分类）；
  装备 `EquipSlot`、冲突诊断、slot order 排序已进入 engine；
  `CharacterSnapshot` 新增 `attack_bonuses`；literal 与公式型 `BONUS` 均
  进入 typed pipeline；broad oracle 23/23 100%。
  alternate weapon profile / proficiency 类 tag（`ALTCRIT*` /
  `ALTDAMAGE` / `ALTEQMOD` / `ALTTYPE` / `ARMORTYPE` / `BASEQTY` /
  `CHANGEPROF` / `WEAPONBONUS`）已由 P4-EQP-2 全部 typed 消费，三数据集
  pending 为 0。`PREEQUIP*`、EquipMod `DR`/`SR`、`.pcg EQMOD=`、
  `PRECHECKBASE` 已完成。
  CompanionMod 真实数据行已不再覆盖丢失，P4-COMP-1 已完成条件行选择与
  HD/SR/ABILITY/traits/BONUS:AC/STAT 消费。
- **目标**：清零残余 BONUS 与装备类 pending legacy；在 IR/engine 层恢复
  archive `bonus.rs` 的 typed stacking policy（G1），并把装备槽位、冲突、
  顺序、装备加成 typed 化（G2）；两者必须走同一条 modifier/AC 管线。
- **指标**：`BONUS=0`；DS-3E `unsupported_tags == 0`；
  装备类 pending legacy 清零（至少 `EQUIPBUY` / `CONTAINS` / `SLOTS` /
  `LOCATION` / `CHARGES` / `APPLY`，共 177 次）；
  `armor_class` / `touch_ac` / `flat_footed_ac` Java 一致率 ≥95%，
  且 Charlie touch、3e_MonKee、Dave flat-footed 等已知失败 case 清零；
  archive 5 个 bonus 测试 + `inventory` / `equipment_slot_sort` 意图有对应测试。
- **门禁**：新增 IR/engine stacking 与 equipment 测试；broad oracle
  `--check --json` 必须 `ok=true`；curated Java 9/9；
  baseline 只允许向 Java 收敛；coverage gate 扩展 equipment 字段值检查。
- **完成定义**：`ARCHIVE_TEST_MAP.md` G1/G2 标记 migrated；
  `TAG_WORKLIST.md` 更新；AC 三字段达标。
- **依赖/风险**：先冻结 stack key 与 bonus type 语义，再做 `classify(category,
  tag)` 拆分 KIT/装备同名 tag；G2 手工规则多，以 archive 测试和 Java csheet
  为 oracle，不凭直觉补规则。

#### P4-PSI-1 灵能语义闭环

- **状态**：已完成首版。`POWER`-style `SPELL:` 记录（`TYPE:Psionic`）进入
  `SpellDefinition.is_psionic`；`import-report` 新增 `psionic_effects` 指标；
  engine 输出 `power_points` / `manifester_levels` / `spell_dcs` /
  `spell_power_types`，并保持灵能职业不出现在 `caster_levels`；
  `PRECHECKBASE` 已 lower（base save 谓词）。
  验收数据集 DS-PSI（`_rsrd_psionics.pcc`）纳入 coverage gate：
  1,570 entities，1,195 lowered，`psionic_effects` 50，completion 99.1%。
  新增 `tests/psionics.rs`（Psion / Psychic Warrior / Wilder）+ 临时态
  概念测试；P4-PSI-2 已补齐 Wilder golden、`powers_known` 与
  `ADD:SPELLCASTER|Psionic` 的 PRC manifester stacking。
- **目标**：补齐 `PRODUCT_SCOPE.md` 的灵能条目：威力、显能者等级、灵能点、
  威力 DC、已知威力。
- **指标**：至少 3 个灵能职业的 power points / manifester level / DC 有 golden；
  `TYPE:Psionic` 与 `FACT:SpellType|Psionic` 数据全部 lower；
  `psionic_effects` 或等价覆盖率字段冻结基线。
- **门禁**：`pcgen-ir/tests/rsrd_tables.rs` + engine golden；
  broad baseline 无回归。
- **完成定义**：PRODUCT_SCOPE 灵能条目全部有实现和测试；文档更新。
- **依赖/风险**：没有 Java XML 直接字段，主要靠 RSRD 表和 golden fixture；
  需要先稳定 P4-PRE-3 的 spell type 模型。

#### P4-TPL-1 模板语义闭环

- **状态**：已完成 `.pcg` `TEMPLATESAPPLIED`、Template entity 激活、
  `TemplateDefinition` / `CreatureTraits` typed 化，并把 `BONUS:STAT` /
  natural AC / HP 接入 snapshot。Race/Template 上的 CR/DR/SR/NATURALATTACKS
  已分类消费；剩余更完整模板执行效果可后续补强。
- **目标**：模板 typed definitions + 激活 + effects 进入 snapshot；
  同时消费 P4-PRE-4 的模板谓词。
- **指标**：`templates` typed count > 0；模板效果在 snapshot 可见；
  `PRETEMPLATE=0`；属于模板的 CR / DR / SR / NATURALATTACKS 等 tag 明确分类。
- **门禁**：模板 fixture engine 测试；broad baseline 无回归；
  `coverage_gate.py` 断言 `templates > 0`。
- **完成定义**：模板从 import → IR → engine 全链路；文档记录哪些 tag 属模板、
  哪些属 creature traits。
- **依赖/风险**：CR / DR / SR / NATURALATTACKS 不全是模板语义，需逐个归类，
  避免把怪物特征误算成模板。

### P4-C SPELL effect 语义

#### P4-SPELL-1 法术定义 → 可执行 spell effect

- **状态**：S1 已完成。`DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` /
  `ADDSPELLLEVEL` / `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK` 已 typed 或
  消费，`PRESPELL` / `PRESPELLSCHOOL` 已 lower，两个数据集这些 tag 均清零。
  S2 已加入 `spell_effects` 指标与 golden 测试：DS-3E 16、DS-35E 41；
  `CharacterSnapshot.spell_effects` 汇总领域法术与准备法术结果。
  P4-SPELL-2 已消费 `ADDDOMAINS` / `PROHIBITCOST` / `DONOTADD` /
  Spell `VARIANTS`：条件领域进入 `domain_spells`，禁止学派成本进入
  snapshot `prohibited_costs`，三数据集这些 tag pending 为 0。
  预研见 [`docs/P4_SPELL1_PRERESEARCH.md`](docs/P4_SPELL1_PRERESEARCH.md)。
  更完整的法术效果数值应用仍可继续扩展。
- **目标**：建立 spell effect 模型，将 spell 定义降低为可执行 `Effect`，
  并在 `import-report` 新增 `spell_effects` 指标。
- **指标**：`spell_effects > 0` 并冻结基线；
  spell 相关 pending legacy 清零或显式 reclassify：
  `DOMAINS` 172、`SPELLLEVEL` 22、`PROHIBITSPELL` 16、
  `ADDSPELLLEVEL` 8、`KNOWNSPELLSFROMSPECIALTY` 8、`SPELLBOOK` 1，
  共 227 次；curated caster cases 的 spell DC / prepared / caster level 不回归。
- **门禁**：coverage gate 新增 `spell_effects >= baseline`；
  curated Java 9/9；broad baseline 不回归；新增 spell effect 单元测试。
- **完成定义**：模型进入 `pcgen-ir` + engine；`spell_effects` 指标和门禁固化；
  `TAG_WORKLIST.md` 更新。
- **依赖/风险**：依赖 P4-PRE-3；`DOMAINS` 可能同时属于 domain 语义，
  需要先归类再 lower。

### P4-D 范围决策

#### P4-DEC-1 KIT / COMPANIONMOD / SIZE

- **目标**：对 KIT / COMPANIONMOD / SIZE 做 in / out / later 决策。
- **指标**：三项各自有明确结论、数据量、用户价值、实现成本；
  `PRODUCT_SCOPE.md` 同步更新。
- **门禁**：文档审查；决策后把排除项加入 allowlist 或明确 non-goal。
- **完成定义**：不再有“先决定是否属于产品范围”的开放项。
- **依赖/风险**：SIZE 可能已部分被 size 谓词使用；要区分“size 数值模型”
  与“SIZE tag 迁移”。

#### P4-DEC-2 G3：XP 表 / point buy

- **目标**：按 `ARCHIVE_TEST_MAP.md` G3 明确 XP 表 / point buy 的去留。
- **指标**：G3 从 gap 变为 dropped / out-of-scope，并写入 PRODUCT_SCOPE 非目标。
- **门禁**：文档同步。
- **完成定义**：G3 更新；无悬挂的 archive 测试意图。
- **依赖/风险**：无。

#### P4-DEC-3 Pathfinder 1e profile

- **目标**：为 Pathfinder 1e profile 给出明确入口或正式延后决定。
- **指标**：至少定义数据来源、profile 范围、首个 acceptance case；
  或明确不进入当前路线图。
- **门禁**：若纳入，新增 profile smoke；若延后，PRODUCT_SCOPE 标注。
- **完成定义**：PRODUCT_SCOPE 与 ROADMAP 一致。
- **依赖/风险**：profile 可能引入新的 tag / class 模型，需单独评估。

#### P4-DEC-4 G4：旧 DataStore

- **目标**：确认 G4 已由 IR 取代，无需实现。
- **指标**：`ARCHIVE_TEST_MAP.md` G4 保持 superseded / no implementation needed。
- **门禁**：文档同步。
- **完成定义**：不再作为未决 gap。
- **依赖/风险**：无。

### P4-E Java 一致率驱动的附加任务

#### P4-DATA-1 怪物 HD progression（JimDop / CloudGiant）

- **状态**：验收已达成。`35e_JimDop` / `35e_CloudGiantHalfDragon` 已与 Java
  全字段 exact；`known_diverging 2→0`，`java_compared 21→23`。
  底层 monster HD 泛化仍可在 P4-RACE-1 中继续补强。
- **目标**：建模 monster Hit Dice progression，让
  `35e_JimDop` / `35e_CloudGiantHalfDragon` 与 Java 一致。
- **指标**：broad oracle `known_diverging` 2→0；`java_compared` 21→23；
  `hit_points` / `final_stats` / AC 一致率显著上升，两个 case 目标 exact；
  cohort expectation 改为 `computed`，baseline 重新生成。
- **门禁**：`broad_oracle.py --check --json` 要求 `ok=true` 且
  `known_diverging=0`；coverage gate 分母守卫保留；broad baseline 更新为 Java exact。
- **完成定义**：两 case Java exact；`known_data_gaps` 删除；
  ROADMAP / TAG_WORKLIST 更新。
- **依赖/风险**：monster HD progression 可能影响其他 creature case；
  先以这两个 Java csheet 为 oracle。

### P4-F 新增域（由阶段 0 triage 派生）

> 来源：[`docs/TAG_TRIAGE.md`](docs/TAG_TRIAGE.md)。目标是让每个 pending tag
> 都有 owner。每项沿用同一验收模板：目标 / 指标 / 门禁 / 完成定义 / 依赖。

- **P4-KIT-0 KIT / STARTPACK 设计 spike（已完成，DS-35E 主瓶颈 ~6,875 次）**
  - **状态**：设计 spike 已完成，结论见
    [`docs/P4_KIT0_DESIGN.md`](docs/P4_KIT0_DESIGN.md)：先做声明式 typed 导入
    （P4-KIT-1），运行时确定性随机展开单独立项（P4-KIT-2，需产品确认）。
  - 目标：先判定 KIT/STARTPACK 应作为“声明式定义导入并统计”，还是“角色创建期
    确定性随机展开”；若进入运行时，再定义 `OPTION`/`LOOKUP`/`VALUES` 权重、
    `QTY:roll(...)`、`EQUIPBUY` 与 `APPLY:INSTANT` 的确定性语义、RNG seed、
    fixture 和 Java/PCGen 对照方式。
  - 指标：产出设计文档，明确数据来源、scope、首个 acceptance case、
    coverage 桶与 `classify(category, tag)` 边界；设计评审通过前不承诺全量清零。
  - 门禁：设计文档评审 + kit fixture 设计 + `coverage_gate.py` 双数据集口径；
    baseline 只允许下降。
  - 依赖/风险：与 `P4-ENG-1`、`classify(category, tag)`、`P4-EQP-1` 的同名
    tag 拆分一起做；若 KIT 属创建期 UI/随机表，应考虑单独 coverage 桶或
    allowlist，避免继续主导 `semantic_completion`。
- **P4-KIT-1 声明式 typed 导入（已完成）**
  - **状态**：已完成。新增 `KitDefinition` / `KitEntry`，KIT/STARTPACK 的
    `OPTION` / `LOOKUP` / `VALUES` / `QTY` / `EQUIPBUY` / `APPLY` /
    `LOCATION` 等 tag 已由 typed import 消费；`classify_for_category` 区分
    KIT 与 Equipment 同名 tag。
  - **指标**：DS-35E KIT pending 6,896→0；DS-3E KIT pending 75→0；
    DS-35E `semantic_completion` 45.5%→76.2%；DS-3E 67.6%→68.9%。
  - **门禁**：`pcgen-ir/tests/kit_import.rs`、`tagclass` 单测、
    `coverage_gate.py` 双数据集、broad oracle 不变。
  - **完成定义**：`kits` typed count 进入 import-report；coverage baseline 重冻结。
- **P4-TEMP-1 临时状态（已完成）**（`TEMPBONUS`/`TEMPVALUE`，DS-35E 376 次）：
  已建立 active-state 模型。`TEMPBONUS` lower 为带 `Predicate::ActiveState`
  条件的 `Effect`；`TEMPVALUE` 解析为 `TemporaryValue` 元数据；
  `CharacterDraft.active_states` / `CharacterSnapshot.active_states` 已接入。
  两个数据集 `TEMPBONUS`/`TEMPVALUE` pending 清零。`EQ` 作用域使用
  `Predicate::Equipped` 近似装备类型约束；`%CHOICE` 目前仍按 0 处理，
  留待后续临时值选择 UI 接入。
- **P4-RACE-1 种族 / 生物特征**（`RACESUBTYPE`/`REACH`/`NATURALATTACKS`/
  `UDAM`/`DR`/`SR`/`CR`/`VISION` 等，DS-35E 838 次）。
  已完成 Halfling 种族 trait / `.MOD` ability 默认链、`CreatureTraits`
  typed 化（Race/Template 的 reach / natural attacks / DR/SR/CR / vision 等）。
  Ability / ClassLevel / Equipment 上的同名 trait 已捕获为 metadata，其中
  active owner entity 会汇入 snapshot；`MOVECLONE` 与 EquipMod `DR`/`SR`
  已消费，CompanionMod 条件行选择已由 P4-COMP-1 落地。
- **P4-CLASS-1 / P4-CLASS-2 职业 / 进阶（已完成）**：
  `SKILLMULT`/`LEVELSPERFEAT`/`HITDIE`/`EXCLASS`/`ADDLEVEL`/`EXCHANGELEVEL`
  已进入 typed `ClassDefinition`，并消费到 `ClassProgression`、首级技能点、
  专长槽、HP（HITDIE 平均）、effective levels 与 `class_interactions`。
- **P4-STAT-1 / P4-STAT-2 属性 / 尺寸（已完成）**：
  `DEFINESTAT`/`STATMOD`/`SIZENUM` 已 typed，并消费到属性下限、派生属性
  修正与 `size_rank` / size modifier。
- **P4-PRE-6 谓词补齐（部分完成）**：已完成 `PRESIZEEQ`/`PRESIZEGTEQ`/
  `PRESIZELT`/`PRESIZELTEQ`/`PRESIZEGT`（DS-35E 58 次）与 `PRETEXT` 改判
  allowlist；`PRETEMPLATE`/`PREMOVE` 已在 P4-PRE-4 落地；剩余
  `!PRESUBCLASS`/`PRELANG`/`PREWEAPONPROF` 需要新运行时状态，分别并入
  P4-CLASS-1 / P4-RACE-1 / P4-EQP-1；`PRESPELL`/`PRESPELLSCHOOL` 并入
  P4-SPELL-1。
- **P4-FAITH 信仰小项（已完成）**：
  `DEITYWEAP` 已消费；`VALIDFORDEITY`/`VALIDFORFOLLOWER` 已进入 deity /
  alignment 信仰校验诊断。

### P4-G DS-35E unsupported owner 表（已清零）

DS-35E 原 `unsupported_tags=104`（pending 102 + allowlisted 2）。下表把
每个 unsupported pending tag 明确到 owner；目前 `unsupported_pending=0`
（仅 `PRETEXT` 2 个 allowlisted）。下表保留 owner 记录，后续门禁可把
DS-35E unsupported 阈值收紧到 0。

| tag | count | owner | 目标 / 门禁 |
|---|---:|---|---|
| `PREMULT` | 40 | P4-PRE-6 | 内层谓词补齐后自动 lower；zero-tag 断言 |
| `BONUS` | 45 | P4-BONUS-1 / P4-EQP-1（合并验收） | 条件形式与装备/加值 target typed 化；`BONUS=0` |
| `!PRESUBCLASS` | 8 | P4-CLASS-1 | 建立 subclass/substitution 状态；`!PRESUBCLASS=0` |
| `PRESPELL` | 4 | P4-SPELL-1 | 法术定义 → executable spell effect；`PRESPELL=0` |
| `PRESPELLSCHOOL` | 3 | P4-SPELL-1 | 法术学派状态；`PRESPELLSCHOOL=0` |
| `PRELANG` | 1 | P4-RACE-1 | 语言集合进入 race state；`PRELANG=0` |
| `PREWEAPONPROF` | 1 | P4-EQP-1 | 武器熟练状态；`PREWEAPONPROF=0` |
| `PRETEXT` | 2 | allowlisted | 纯展示文本，已不计入 pending；保持 allowlist |

> DS-3E 的残余 `BONUS=3` 同样由 P4-BONUS-1 / P4-EQP-1 合并验收清零，
> 届时两个数据集都应达到 `unsupported_tags == 0`。

### P4-H 剩余 creature-trait owner 表

Race/Template 上的 `VISION` / `DR` / `SR` / `NATURALATTACKS` / `REACH` /
`RACESUBTYPE` / `UDAM` / `UMULT` 等已 typed 化，并进入 `CharacterSnapshot`
的 `vision` / `damage_reduction` / `spell_resistance` / `natural_attacks`。
剩余同名 tag 出现在 Ability / ClassLevel / Equipment 等 category，已统一
captured 到 `CompiledRulePack.creature_trait_records`；其中 active
ClassLevel / Ability / Feat / Equipment 已汇入 snapshot；`MOVECLONE` 与
EquipMod `DR`/`SR` 已有 runtime 消费。CompanionMod 的 `FOLLOWER:` 行已按
id 分组保存，并已由 P4-COMP-1 消费 `PRERACE` 条件、HD/SR/ABILITY/traits
与 BONUS:AC/STAT：

| category | tags / 次数 | owner | 处理方式 |
|---|---|---|---|
| Ability | `UDAM` 54、`UMULT` 54、`NATURALATTACKS` 18、`VISION` 9、`DR` 6、`SR` 3、`MOVECLONE` 1 | P4-RACE-1 / P4-CLASS-1 | 怪物/种族 trait 归 P4-RACE-1；职业能力归 P4-CLASS-1，后续汇入 `CreatureTraits` |
| ClassLevel | `UDAM` 20、`UMULT` 1 | P4-CLASS-1 | 职业进阶表能力，进入 class feature / unarmed damage 模型 |
| Equipment | `REACHMULT` 7、`SR` 5、`DR` 3 | P4-EQP-1 | 装备特殊能力，接入装备 trait 输出 |
| EquipMod | `DR` 4、`SR` 6 | P4-EQP-1 | 装备 modifier 特殊能力 |
| CompanionMod | `SR` 4 | P4-COMP-1 | ✅ 行级 PRERACE 选择 + HD/SR/ABILITY/traits/BONUS:AC/STAT 消费 |

> 产品输出范围已冻结：Race/Template 的视觉、DR、SR、natural attacks、reach、
> unarmed damage 进入 `CharacterSnapshot`；其余 category 的 trait 已保存为
> typed metadata，active owner entity 会汇入 snapshot，未实现消费的仍为 pending。

### P4 推荐执行顺序

已完成：P4-PRE-1、P4-PRE-2、P4-PRE-5、P4-ENG-1、P4-KIT-0、P4-KIT-1、P4-DATA-1。

> 阶段 0 口径冻结已完成：DS-3E 作回归代理、DS-35E 作主验收，
> 两个数据集各自冻结 coverage 基线（见 `docs/TESTING.md`）。
> 后续按下表推进功能。

| 顺序 | 任务 | 说明 |
|---:|---|---|
| ~~0~~ | ~~P4-PRE-3~~ | ✅ 已完成：`SpellPowerType` + 法术书，unsupported 25→16 |
| ~~0b~~ | ~~P4-PRE-6（部分）~~ | ✅ size 谓词 + `PRETEXT`；剩余并入 TPL/EQP/RACE |
| ~~0c~~ | ~~P4-ENG-1~~ | ✅ 已完成：class/level 激活 + 默认 BAB=0 + modifier 归一化；BAB 21/21 |
| ~~0d~~ | ~~P4-KIT-0~~ | ✅ 设计 spike 完成：声明式导入优先，随机展开单独立项 |
| ~~1~~ | ~~P4-KIT-1~~ | ✅ 完成：KIT pending 6,896→0；DS-35E completion 45.5%→76.2% |
| 2 | P4-BONUS-1 + P4-EQP-1/P4-EQP-2 | ✅ 统一 `TypedBonus`、EquipSlot/冲突/排序、PREEQUIP、EquipMod DR/SR、alternate weapon profile / WEAPONBONUS / PREWEAPONPROF |
| 3 | P4-SPELL-1 / P4-SPELL-2 | ✅ S1/S2 spell_effect + `ADDDOMAINS`/`PROHIBITCOST`/`DONOTADD`/`VARIANTS` 消费；法术数值执行可继续扩展 |
| ~~4~~ | ~~P4-DATA-1~~ | ✅ 完成：known_diverging 0；23/23 Java exact |
| 4b | P4-RACE-1 / P4-TPL-1 / P4-COMP-1 | ✅ Race/Template typed + active owner trait 汇入 snapshot；MOVECLONE 与 EquipMod DR/SR 已消费；CompanionMod PRERACE/BONUS/traits runtime 已完成 |
| 5 | P4-TEMP-1 / P4-PSI-1/P4-PSI-2 | ✅ P4-TEMP-1 active-state；Psion/Psychic Warrior/Wilder golden、`powers_known`、PRC manifester stacking 完成 |
| 6 | P4-CLASS-1/2、P4-STAT-1/2、P4-FAITH-1/2 | ✅ typed + runtime 消费完成：技能点/专长/HP/ADDLEVEL/EXCLASS/EXCHANGELEVEL、属性下限/STATMOD/SIZENUM、信仰校验 |
| 7 | P4-DEC-2 / DEC-4 | G3 dropped、G4 归档 |
| 8 | P4-ABILITY-1 | ✅ `AbilityDefinition` / `AbilityCategoryDefinition`、`ABILITYLIST`、`POOL`、`STACK`/`MULT`、`draft.choices` 解析与 `unresolved_choices` diagnostics；选择类 pending 清零 |

> 每个任务合并前必须同时满足：`cargo test --workspace`、
> `coverage_gate.py` 严格模式、`broad_oracle.py --check --json`；
> 涉及 engine 行为时还要跑 archive differential。baseline 只允许向 Java 收敛。

## 门禁命令

```bash
bash scripts/check.sh                 # 开发全量（有数据时自动严格）
python3 scripts/coverage_gate.py      # 严格；含 DS-3E / DS-35E 非回归基线；缺数据退出码 2
python3 scripts/coverage_gate.py --write-coverage-baseline   # 仅评审后重置基线
python3 scripts/broad_oracle.py --check --json
python3 scripts/broad_oracle.py --report
cargo run -p pcgen-cli -- import-report <pcc> --histogram --top 40
```

> DS-35E 主验收报告：
> `cargo run -p pcgen-cli -- import-report data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc --json`
