# Archive 测试 → 新测试对照表

> 目的：确认旧实现（`reference/pcgen-rust-archive`）的 114 个测试行为**没有
> 静默丢失**。旧实现的运行路径已被删除，本表记录每个测试域的去向。
>
> 统计口径：archive `#[test]` 共 **114** 个（core 54 / data 45 / formula 3 /
> output 12 / cli 0）。新工作区当前 **117** 个（P4-ENG-1 后）。
>
> 分类：
> - **migrated**：行为在新工作区有对应测试。
> - **superseded**：旧测试针对已删除的实现细节，其**意图**由新架构测试覆盖。
> - **dropped**：有意不再支持，需说明理由。

## 1. pcgen-core（archive: 54）

| archive 测试 | 状态 | 新位置 |
|---|---|---|
| `algorithms`: `topological_sort_is_stable` / `rejects_cycles` / `scc_finds_cycles_and_singletons` | migrated | `pcgen-core/src/algorithms.rs` 同名 + `scc_of_dag_...` |
| `bonus` (5 个 stacking/policy 测试) | superseded | `pcgen-engine/tests/external_modifiers.rs` + `effects.rs`（IR 层执行）；stacking policy 逻辑待迁移（见差距 G1） |
| `character/ability_scores` (2) | superseded | `pcgen-engine/tests/snapshot_summary.rs` + `pcgen-core/src/vocab.rs` |
| `character/advancement` (2) | migrated | `pcgen-core/src/advancement.rs` 同名 |
| `character/bonuses` (1) | superseded | `pcgen-engine/tests/external_modifiers.rs` |
| `character/combat` (1) | superseded | `pcgen-engine/tests/snapshot_summary.rs` |
| `character/inventory` (1) | **dropped** | 装备槽位/冲突规则未迁移（差距 G2） |
| `character/progression` (3) | superseded | `pcgen-engine` progression + `pcgen-ir/tests/rsrd_tables.rs` |
| `character/skills` (2) | migrated | `pcgen-import/tests/golden_pcg.rs::figfae_golden_preserves_half_rank` + `snapshot_summary` |
| `character/spells` (1) | superseded | `pcgen-engine` spell_dcs + `pcgen-output` goldens |
| `choice` (3) | migrated | `pcgen-engine/tests/effects.rs`（Choose 执行）+ `pcgen-core/src/vocab.rs` |
| `class` (2) | migrated | `pcgen-ir/tests/rsrd_tables.rs` + `pcgen-ir/src/typed.rs` |
| `equipment` (2) | superseded | `pcgen-ir/src/typed.rs` + `pcgen-engine` external modifiers |
| `game_mode` (2) | **dropped** | XP 表 / point buy 不在 `PRODUCT_SCOPE`（见 G3） |
| `gestalt` (5) | migrated | `pcgen-core/src/gestalt.rs` 5 个同名 |
| `identity` (2) | migrated | `pcgen-core/src/identity.rs` 同名 |
| `prereq` (2) | superseded | `pcgen-engine/tests/effects.rs`（内嵌 PRE 校验）+ `import_smoke` |
| `psionics` (3) | migrated | `pcgen-core/src/psionics.rs` `rsrd_tables_match_known_values` + `import` typed |
| `race` (1) | superseded | `pcgen-ir/src/typed.rs` + `snapshot_summary` |
| `rational` (2) | migrated | `pcgen-core/src/rational.rs` 同名 |
| `skill` (3) | superseded | `figfae_golden_preserves_half_rank` + `pcgen-output` |
| `spell` (3) | superseded | `pcgen-ir/src/typed.rs` + `pcgen-output` goldens |
| `variable_graph` (1) | migrated | `pcgen-engine/tests/effects.rs::variable_graph_resolves_...` |

## 2. pcgen-data（archive: 45）

| archive 测试 | 状态 | 新位置 |
|---|---|---|
| `character_builder` (1) | superseded | `pcgen-engine/tests/*` + `golden_pcg` |
| `dataset` (1) | **dropped** | 旧 `DataStore` 模型已删除（见 G4） |
| `import` (2) | migrated | `pcgen-ir/tests/import_smoke.rs` + `histograms.rs` |
| `lst` (1) | superseded | `pcgen-ir/src/typed.rs` + `pcgen-import/src/tokenizer.rs` |
| `pcc` (1) | migrated | `pcgen-import/src/pcc.rs::parse_minimal_pcc` |
| `pcg` (1) | migrated | `pcgen-import/src/pcg.rs::parses_core_pcg_inputs` |
| `rule_context` (8) | superseded | `pcgen-engine/tests/effects.rs` + `advanced_rules.rs`（IR 执行取代 DataStore 路径） |
| `store` (6) | migrated | `pcgen-ir/src/patched.rs`（MOD/CLEAR/FORGET/COPY）+ `patches` |
| `tokenizer` (2) | migrated | `pcgen-import/src/tokenizer.rs` 同名 |
| `advancement_seam` (2) | migrated | `pcgen-core/src/advancement.rs` |
| `campaign_mini` (1) | migrated | `pcgen-import/src/loader.rs::loads_mini_campaign_records` + coverage gate |
| `gestalt_fixture` (1) | migrated | `pcgen-engine/tests/gestalt_engine.rs` |
| `gestalt` (3) | migrated | `pcgen-core/src/gestalt.rs` + `advanced_rules.rs` |
| `golden_pcg` (5) | migrated | `pcgen-import/tests/golden_pcg.rs` 5 个同名 |
| `pcc_graph` (2) | migrated | `pcgen-import/tests/pcc_graph.rs` 同名 |
| `pcg_gestalt_roundtrip` (1) | migrated | `pcgen-import/src/pcg.rs::gestalt_slot_history_round_trips` |
| `psionics` (2) | superseded | `pcgen-core/src/psionics.rs` + `pcgen-ir/src/typed.rs` |
| `real_campaign` (4) | migrated | `pcgen-import/tests/rsrd_tables.rs` + `real_import.rs`；Java baseline 部分由 `coverage_gate.py` curated 9 例承接 |
| `snapshot_oracle` (1) | superseded | `coverage_gate.py` + `broad_oracle.py` |

## 3. pcgen-formula（archive: 3）

| archive 测试 | 状态 | 新位置 |
|---|---|---|
| `formula` (3) | migrated | `pcgen-formula/src/lib.rs` 同名 |

## 4. pcgen-output（archive: 12）

| archive 测试 | 状态 | 新位置 |
|---|---|---|
| `output_formats` | migrated | `pcgen-output/tests/render_smoke.rs::text_snapshot_renders` |
| `summary_drives_combat_and_spell_output` | superseded | `text_snapshot_renders` + goldens |
| `template_subset_renders_summary` | superseded | `official_subset_golden` |
| `advanced_ftl_assign_macro_and_string_format` | migrated | `macro_and_include_render` + `official_subset_golden` |
| `ftl_builtins_and_function_calls` | superseded | `official_subset_golden` |
| `executes_real_official_pcgen_sheet_if_present` | migrated | `real_official_sheet_smoke` |
| `official_pcgen_output_sheet_subset` | migrated | `official_subset_golden` |
| `template_namespace_import` | migrated | `macro_and_include_render` |
| `template_file_include` | migrated | `macro_and_include_render` |
| `output_sheet_golden_xml` | migrated | `official_subset_golden` |
| `gestalt_plan_renders_in_sheets` | superseded | `text_snapshot_renders`（gestalt 段） |
| `equipment_slot_sort` | **dropped** | 依赖未迁移的槽位模型（G2） |

## 5. 差距与结论

| id | 差距 | 影响 | 建议 | P4 任务 |
|---|---|---|---|---|
| G1 | stacking policy（`bonus.rs` 5 测试）未在 IR 层完整重建 | 多类型加值叠加可能不准 | P4 与 BONUS 家族一起迁移 | `P4-BONUS-1` |
| G2 | 装备槽位/冲突/排序未迁移 | inventory 与 sheet 排序 | 归入 P4「装备语义」 | `P4-EQP-1` |
| G3 | XP 表 / point buy 未迁移 | 不在 PRODUCT_SCOPE | **P4-DEC-2 结论：dropped（非目标）** | `P4-DEC-2` ✅ |
| G4 | 旧 `DataStore` 模型删除 | 无直接替代 | **P4-DEC-4 结论：superseded，无需实现** | `P4-DEC-4` ✅ |
| G5 | KIT / COMPANIONMOD / SIZE 范围未定 | 影响 DS-35E 覆盖分母 | **P4-DEC-1 结论：KIT / COMPANIONMOD in scope，计入分母；SIZE 数值模型保留，SIZE tag 迁移并入门/移动与装备任务** | `P4-DEC-1` ✅（SIZE 实现待做） |

## 6. archive differential 复跑结果（三方差分）

`scripts/snapshot_diff_archive.py` 现在是 **archive vs new vs Java** 的三方差分，
当 archive 与 new 不一致时由 **Java PCGen csheet 裁决**：

- new == java → `ARCHIVE-BUG`（archive 自身有 bug，不算失败）
- archive == java → `REGRESSION`（新引擎回归，失败）
- 都不一致 → `DIVERGENCE`（失败）

8 个 case 的结果：

| case | 字段 | archive | new | java | 判定 |
|---|---|---:|---:|---:|---|
| `3e_BarJack.pcg` | `flat_footed_ac` | 16 | 19 | **19** | archive-bug（未处理 uncanny dodge）|
| `3e_WizShar.pcg` | `skills.literacy` | 3 | 1 | **1** | archive-bug（错误叠加 synergy）|

**结论：8 个 case 无新引擎回归，0 divergence**；旧 archive 有 2 处自身 bug，
新引擎在两处都与 Java 一致。

**结论**：114 个测试中，**无一个属于"未决的静默丢失"**——每个都归类为
migrated / superseded / dropped；dropped 与差距都有明确记录，且 archive
differential 现已作为 strict 门禁运行。当前 117 个测试是不同粒度
（IR + 门禁 + differential）的结果，不应与 114 直接比数量。
