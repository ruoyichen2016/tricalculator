# PCGen-Rust Roadmap

> 本文档是路线与状态的**唯一事实源**。Sprint 历史见 `MIGRATION.md`，
> 测试/门禁与数据版本见 `docs/TESTING.md`，待迁移 tag 见 `docs/TAG_WORKLIST.md`。
> 数字来自固定数据 `scripts/testdata.lock.json`
> （ref `f91bd03f2dbe1145d5526b52aa3e48fcd0f2fc8d`）。

## 当前状态（最近验证：固定数据 + 全量门禁）

- 工作区：7 crate（core / formula / ir / import / engine / output / cli）。
- 测试：`cargo test --workspace` **122 passed**。
- DS-3E（回归代理，`srdbasic.pcc`）：3,779 entities，23,723 tag occurrences，
  lowered 3,108；legacy pending 1,398；unsupported 3；
  **semantic completion 68.9%**。
- DS-35E（主验收，`_rsrd_basics.pcc`）：15,741 entities，
  lowered 7,814；legacy pending 2,346；unsupported 104；
  **semantic completion 76.2%**。
- 门禁 `scripts/coverage_gate.py`：**12 pass / 0 fail / 0 skip**（严格模式，含双数据集 coverage 基线与 DS-35E 阈值）。
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

## P4：按用户价值迁移语义（进行中）

> P4 已重写为可验收任务。每项必须回答：目标是什么、用什么指标判断、
> 由哪个门禁守住、什么条件下算完成、依赖/风险是什么。
> `docs/ARCHIVE_TEST_MAP.md` 的 G1–G4 已链接到对应任务。
>
> 已完成基线：`CATEGORY` ability pool（941→0）、`PREMULT` 主体（113→1）、
> BONUS 家族主体（167→4）、5 个 data-missing case 修复（0 data-missing）。
> 以下只列仍未完成的任务。

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

- **状态**：第一阶段已完成（typed numeric AC、默认豁免零值、flat-footed 负 DEX、
  per-level HP minimum、Halfling 种族 trait、template stat/AC/HP）。
  broad oracle 全部字段 23/23（100%），`known_diverging=0`；测试 119→122。
  剩余：DS-3E/DS-35E `BONUS` unsupported、装备 pending legacy、完整 stacking
  policy 与装备槽位语义。
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

- **状态**：第一阶段完成。已解析 `.pcg` `TEMPLATESAPPLIED`、激活 Template
  entities，并把 `BONUS:STAT` / `BONUS:COMBAT|AC|TYPE=NaturalArmor` 接入
  snapshot；Half-Dragon 的 STR/CON/INT/CHA、natural AC、HP 已与 Java exact。
  剩余 CR/DR/SR/NATURALATTACKS 分类与更完整模板效果。
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
- **P4-TEMP-1 临时状态**（`TEMPBONUS`/`TEMPVALUE`，DS-35E 376 次）：
  active-state 模型（Rage 等），不是静态 snapshot。
- **P4-RACE-1 种族 / 生物特征**（`RACESUBTYPE`/`REACH`/`NATURALATTACKS`/
  `UDAM`/`DR`/`SR`/`CR`/`VISION` 等，DS-35E 838 次）。
  已完成 Halfling 种族 trait / `.MOD` ability 默认链，修复 3e_SWizSam 豁免；
  剩余种族/生物特征 tag 仍待 lower。
- **P4-CLASS-1 职业 / 进阶**（`SKILLMULT`/`LEVELSPERFEAT`/`HITDIE`/`EXCLASS`
  等，DS-35E 29 次）。
- **P4-STAT-1 属性 / 尺寸**（`DEFINESTAT`/`STATMOD`/`SIZENUM`，DS-35E 46 次）。
- **P4-PRE-6 谓词补齐（部分完成）**：已完成 `PRESIZEEQ`/`PRESIZEGTEQ`/
  `PRESIZELT`/`PRESIZELTEQ`/`PRESIZEGT`（DS-35E 58 次）与 `PRETEXT` 改判
  allowlist；`PRETEMPLATE`/`PREMOVE` 已在 P4-PRE-4 落地；剩余
  `!PRESUBCLASS`/`PRELANG`/`PREWEAPONPROF` 需要新运行时状态，分别并入
  P4-CLASS-1 / P4-RACE-1 / P4-EQP-1；`PRESPELL`/`PRESPELLSCHOOL` 并入
  P4-SPELL-1。
- **P4-FAITH 信仰小项**（`DEITYWEAP`/`VALIDFORDEITY`/`VALIDFORFOLLOWER`，
  DS-35E 5 次）。

### P4-G DS-35E unsupported owner 表

DS-35E 当前 `unsupported_tags=104`（pending 102 + allowlisted 2）。下表把
每个 unsupported pending tag 明确到 owner；全部清零后再把 coverage gate 的
DS-35E 阈值收紧到 0，并补 zero-tag 断言。

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
| 2 | P4-BONUS-1 + P4-EQP-1 第二阶段 | 清残余 BONUS/装备 pending；完整 stacking policy 与槽位/冲突语义 |
| 3 | P4-SPELL-1 | 清 DOMAINS/SPELLLEVEL/PRESPELL*；可与第 2 项并行 |
| ~~4~~ | ~~P4-DATA-1~~ | ✅ 完成：known_diverging 0；23/23 Java exact |
| 4b | P4-RACE-1 / P4-TPL-1 | 第一阶段完成 Halfling trait / Half-Dragon template；剩余种族/模板语义 |
| 5 | P4-TEMP-1 / P4-PSI-1 | active-state 与灵能闭环；可与第 2–4 项并行 |
| 6 | P4-CLASS-1 / P4-STAT-1 / P4-FAITH | triage 派生小项；清 `!PRESUBCLASS` 等 unsupported |
| 7 | P4-DEC-2 / DEC-4 | G3 dropped、G4 归档 |

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
