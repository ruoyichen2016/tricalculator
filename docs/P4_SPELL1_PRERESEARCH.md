# P4-SPELL-1 预研：法术定义 → 可执行 spell effect

> 状态：S1 typed 定义与覆盖率已完成；S2 执行层待做。
> 数据：DS-35E `_rsrd_basics.pcc` 固定数据，逐 category pending 探针。
> 关联任务：`ROADMAP.md` P4-C / P4-SPELL-1。

## 1. 数据画像

DS-35E 中与法术/领域相关的 pending 主要集中在：

| tag | category 分布 | 合计 |
|---|---|---:|
| `DOMAINS` | Spell 186、Deity 1 | 187 |
| `SPELLLEVEL` | ClassLevel 50、Ability 44、Domain 35 | 129 |
| `PROHIBITSPELL` | Class 24 | 24 |
| `ADDSPELLLEVEL` | Ability 14 | 14 |
| `KNOWNSPELLSFROMSPECIALTY` | Class 8 | 8 |
| `SPELLBOOK` | Class 1 | 1 |
| unsupported `PRESPELL` | Ability 2、Class 2 | 4 |
| unsupported `PRESPELLSCHOOL` | Class 3 | 3 |

这些数据分别覆盖：

- 领域法术列表（`DOMAINS`）；
- 法术等级/职业法术表（`SPELLLEVEL`）；
- 禁止学派/法术（`PROHIBITSPELL`）；
- 额外法术位与专精已知法术（`ADDSPELLLEVEL` / `KNOWNSPELLSFROMSPECIALTY`）；
- 法术书与准备施法状态（`SPELLBOOK`）；
- 法术类型/学派谓词（`PRESPELL` / `PRESPELLSCHOOL`）。

## 2. 建议模型

在现有 `SpellDefinition` 基础上分三层：

1. **定义层**：`SpellDefinition` 增加 school/descriptor/level/class lists 等 typed 字段。
2. **授予层**：把 `DOMAINS` / `SPELLLEVEL` / `ADDSPELLLEVEL` /
   `KNOWNSPELLSFROMSPECIALTY` 降低为 `EffectAction::GrantSpell` 或 typed
   spellbook/domain 条目。
3. **执行层**：`CharacterSnapshot` 的 `spell_dcs` / `prepared_spells` /
   `caster_levels` 保持现有输出，并新增 `spell_effects` 统计与执行轨迹。

`PRESPELL` / `PRESPELLSCHOOL` 继续走 `Predicate` 模型，但需要：
- 独立于 `CastingType` 的 `SpellPowerType`（已有）；
- spell school 状态；
- prepared / spontaneous / spellbook 三种模式的状态。

## 3. 分阶段验收

### S1：typed 定义与覆盖率

- `DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` / `ADDSPELLLEVEL` /
  `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK` 从 pending 清零或显式 reclassify。
- `import-report` 新增 `spell_effects` typed count 并冻结基线。
- broad oracle / archive differential 不变。

### S2：执行与 Java 字段

- `PRESPELL` / `PRESPELLSCHOOL` 清零。
- curated caster cases 的 `spell_dcs` / `prepared_spells` / `caster_levels`
  不回归。
- 至少一个领域法术 / 专精法术 golden 进入 engine 测试。

## 4. 风险

- `DOMAINS` 同时属于 domain 语义和 spell list 语义，需先分类再 lower。
- 法术效果与现有 prepared spells / caster levels 的时序需要冻结。
- `SPELLLEVEL` 在 ClassLevel / Ability / Domain 三类下语义不同，不能只按 tag
  名处理。

## 5. 下一步

建议 P4-SPELL-1 与 P4-BONUS/EQP 第二阶段并行启动，先做 S1 typed 定义与覆盖率，
再做 S2 执行与 Java 字段。该预研不阻塞当前主线。
