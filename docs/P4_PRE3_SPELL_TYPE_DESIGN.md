# 法术类型 / 法术书设计（P4-PRE-3）

> 范围：`PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST` / `PRESPELLSCHOOL`，
> 以及数据侧的 `SPELLBOOK:YES`、`FACT:SpellType|...`。
> 关联任务：解锁 `P4-SPELL-1`，配合 `P4-PSI-1`。
> 数据口径：DS-3E `srdbasic.pcc`（回归代理）+ DS-35E `_rsrd_basics.pcc`（主验收）。
>
> **状态：已实现**（`SpellPowerType` / `HasSpellbook` / `CastingTypeIs` / `HasSpellPower`
> + 阶段 A 派生；DS-3E unsupported 25→16，DS-35E 276→152）。

## 0. 结论

1. 引入**独立于 `CastingType` 的新概念** `SpellPowerType`
   `{ Arcane, Divine, Psionic, Unknown }`。`CastingType`（Prepared/Spontaneous/Innate）
   表达“怎么施法”，`SpellPowerType` 表达“施什么能量”，两者不能互相推导。
2. 导入层从 CLASS 记录派生：
   - `FACT:SpellType|Arcane|Divine|Psionic` → `ClassDefinition.spell_power`；
   - `SPELLBOOK:YES` → `ClassDefinition.requires_spellbook`。
3. 引擎在**谓词求值前**派生 `SpellcastingPrereqState`（每 class 的施法者等级、
   每 `SpellPowerType` 的最高可施法等级与可用法术位、是否有法术书），
   与 `P4-PRE-2` 的 `derive_prereq_bab()` 同类，避免循环。
4. 新增谓词 `SpellType { power, min_level, count }`、`HasSpellbook`、
   `CastingTypeIs`；lowering 覆盖 `PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST`。
   `PRESPELLSCHOOL` 因语义待确认，放二期。
5. v1 **不做**“effect 动态改变施法者等级”的完整不动点：spell 谓词只从
   class levels + progression 表派生；动态部分记入开放问题并给诊断。

## 1. 目标与指标

| 项 | 目标 |
|---|---|
| 目标 tag（DS-3E / DS-35E 次数） | `PRESPELLTYPE` 7/23、`PRESPELLBOOK` 2/0、`PRESPELLCAST` 0/2、`PRESPELLSCHOOL` 0/3 |
| 指标 | 前三个 tag 在两个数据集均为 0；DS-3E `unsupported_tags <= 16`；DS-35E 有意义下降 |
| 门禁 | `predicate_lowering` 单测；engine caster golden；coverage gate 双数据集 `zero_tags`；curated Java 9/9；broad oracle baseline 不漂移 |
| 完成定义 | 模型进入 `pcgen-ir`（导入）+ `CharacterSnapshot`（派生）；`ARCHITECTURE.md` 记录时序；`TAG_WORKLIST.md` / baseline 更新 |

## 2. 真实数据证据

```text
CLASS:Bard      FACT:SpellType|Arcane      CLASS:Wizard     SPELLBOOK:YES
CLASS:Cleric    FACT:SpellType|Divine      Arcane Fire      PRESPELLTYPE:1,Arcane=9
CLASS:Sorcerer  FACT:SpellType|Arcane      Loremaster       PRESPELLSCHOOL:1,Divination=3
CLASS:Druid     FACT:SpellType|Divine      Dragon Disciple  PRESPELLCAST:MEMORIZE=N
```

- `PRESPELLTYPE` 共 30 次，`1,Arcane=9` / `1,Divine=7` 等，主要出现在
  epic / prestige ability 上；语义是“能施放 n 个某类型、某等级的法术”。
- `PRESPELLBOOK:YES` 共 2 次（`Lucubration`、`Mnemonic Enhancer`），
  要求施法者使用法术书（Wizard 型）。
- `PRESPELLCAST:MEMORIZE=N` 共 2 次（`Dragon Disciple`），要求非准备施法。
- `SPELLBOOK:YES` 在数据侧标记 Wizard 类；`FACT:SpellType|...` 标记职业能量类型。

## 3. 现有模型与缺口

| 位置 | 现状 | 缺口 |
|---|---|---|
| `pcgen_core::CastingType` | `Prepared / Spontaneous / Innate / Unknown` | 不表达 Arcane/Divine/Psionic |
| `ClassDefinition` | `spellcasting_ability`、`casting_type`、`spell_slots`、`spells_known`、`spell_list` | 缺 `spell_power`、`requires_spellbook` |
| `SpellDefinition` | 有 `is_psionic` | 只有 spell 侧 psionic 标记，无 power type |
| `engine::compute_spellcasting` | 在 `evaluate` 内产出 caster levels / slots / DC / prepared | 谓词求值早于/独立于它，需前置派生 |
| `Predicate` | 无任何 spell 变体 | 需新增 `SpellType` / `HasSpellbook` / `CastingTypeIs` |

`FACT:SpellType|Psionic` 在 3e/35e basics 中不出现（在灵能数据里），
但解析与谓词应与 `P4-PSI-1` 共用同一枚举。

## 4. 推荐方案

### 4.1 数据模型

```rust
// pcgen-core
pub enum SpellPowerType { Arcane, Divine, Psionic, Unknown }

pub struct ClassDefinition {
    // ... existing ...
    pub spell_power: SpellPowerType,   // from FACT:SpellType|...
    pub requires_spellbook: bool,      // from SPELLBOOK:YES
}

pub struct CharacterSnapshot {
    // ... existing ...
    pub spell_power_types: BTreeSet<SpellPowerType>,
    pub max_spell_level: BTreeMap<SpellPowerType, u32>,
    pub has_spellbook: bool,
}
```

`CharacterDraft` **不新增输入**：法术书与法术类型都是职业/规则的派生结果，
不写回存档（符合架构原则 2）。

### 4.2 导入 lowering

- `parse_class`：
  - `FACT:SpellType|X` → `spell_power`（大小写不敏感；未知值 → `Unknown`）；
  - `SPELLBOOK:YES` → `requires_spellbook = true`；
  - 复用现有 `is_psionic` 判定（`TYPE:Psionic` 或 `FACT:SpellType|Psionic`）。
- `parse_predicate` 新增：
  - `PRESPELLTYPE:w,type=level` → `Predicate::SpellType { power, min_level: level, count: w }`；
  - `PRESPELLBOOK:YES` → `Predicate::HasSpellbook`；
  - `PRESPELLCAST:MEMORIZE=N/Y` → `Predicate::CastingTypeIs { prepared: false/true }`；
  - `PRESPELLSCHOOL:w,school=level,...` → 二期，先保持 unsupported 并给明确诊断。

### 4.3 求值时序（关键，与 P4-PRE-2 对齐）

```text
阶段 A（不依赖 effects）
  class_levels + ClassDefinition(spell_slots/spells_known/spell_power/requires_spellbook)
    -> derive_spellcasting_prereq_state()
       = per class caster_level
       + per SpellPowerType max_spell_level / slots_at_or_above(level)
       + per class casting_type
       + has_spellbook
阶段 B（effect closure）
  apply_effects 在求值 predicate 前接收阶段 A 的 state
  -> Predicate::SpellType / HasSpellbook / CastingTypeIs 只读该 state
阶段 C（最终 snapshot）
  compute_spellcasting() 复用阶段 A 的派生结果 + 现有 spell DC / prepared 逻辑，
  产出 snapshot.spell_power_types / max_spell_level / has_spellbook
```

`PRESPELLTYPE:w,type=level` 的 v1 判定：

- 存在活跃施法职业 `c`，且 `spell_power(c) == type`、`max_spell_level(c) >= level`；
- 若 `w > 1`，则要求该 power 在 `>= level` 的法术位总数 `>= w`；
- `type == Unknown` 的职业不参与匹配（保守，避免误放行）。

### 4.4 循环防护

- spell 谓词**不参与** `SpellcastingPrereqState` 的派生，因此无自环；
- effect 动态提升施法者等级（如 Practiced Spellcaster）v1 不纳入；
  若检测到相关 effect，输出 `spellcasting_prereq_stale` 诊断而不是猜测；
- 与 `P4-PRE-2` 的 `derive_prereq_bab` 使用同一“阶段 A 只读静态定义”的约定。

## 5. 接口改动清单

| 文件 / 类型 | 改动 |
|---|---|
| `pcgen-core/src/domain.rs` | `SpellPowerType`；`ClassDefinition.spell_power` / `requires_spellbook` |
| `pcgen-core/src/rules.rs` | `Predicate::SpellType` / `HasSpellbook` / `CastingTypeIs` |
| `pcgen-engine/src/snapshot.rs` | `spell_power_types` / `max_spell_level` / `has_spellbook` |
| `pcgen-engine/src/engine.rs` | `derive_spellcasting_prereq_state()`；`compute_spellcasting` 复用它 |
| `pcgen-engine/src/effects.rs` | `EffectState` 注入派生 state；三个新谓词求值 |
| `pcgen-ir/src/rulepack.rs` | `parse_class` + `parse_predicate` 新 arm |
| `pcgen-ir/src/typed.rs` | class 的 `FACT:SpellType` / `SPELLBOOK` 解析 |
| `pcgen-cli` import-report | 可选：输出 `spell_power_types` / `has_spellbook` 计数 |

## 6. 测试与门禁

- `pcgen-ir/tests/predicate_lowering.rs`：
  - `PRESPELLTYPE:1,Arcane=9` → `SpellType { Arcane, 9, 1 }`；
  - `PRESPELLBOOK:YES` → `HasSpellbook`；
  - `PRESPELLCAST:MEMORIZE=N` → `CastingTypeIs { prepared: false }`；
  - 未知 power / 缺等号 → unsupported（不 panic）。
- `pcgen-engine/tests/`：
  - Wizard 10 级：`has_spellbook == true`、`max_spell_level(Arcane) >= 5`；
  - Sorcerer：`has_spellbook == false`、Arcane；
  - Cleric：Divine；
  - 多职业：`spell_power_types` 为并集。
- `coverage_gate.py`：
  - DS-3E `zero_tags += PRESPELLTYPE/PRESPELLBOOK`，`unsupported_tags <= 16`；
  - DS-35E `zero_tags += PRESPELLTYPE`，并把基线向下更新（用新 floor）。
- curated Java differential：`35e_Bard` / `3e_BrdJoe` 的
  `spell_dcs` / `prepared_spells` / `caster_levels` 保持 exact；
- broad oracle `--check --json` 必须 `ok=true`，baseline 无漂移。

## 7. 完成定义

- DS-3E / DS-35E：`PRESPELLTYPE`、`PRESPELLBOOK`、`PRESPELLCAST` 为 0；
- `SpellPowerType` 进入 `pcgen-ir` + `CharacterSnapshot` 并序列化稳定；
- `ARCHITECTURE.md` 记录阶段 A/B/C 时序与“只读静态定义”的约定；
- `TAG_WORKLIST.md` / `TAG_TRIAGE.md` / coverage baseline 同步更新。

## 8. 开放问题

1. `PRESPELLSCHOOL:w,school=level,...` 的精确语义（`w` 是法术数还是学校数）
   需查 Java 源码 / 用 csheet 验证，v1 先不实现。
2. effect 动态施法者等级（Practiced Spellcaster、进阶职业叠级）何时进入阶段 A；
   是否需要第二轮不动点。
3. Domain 法术是否一律 `Divine`，还是跟随所属职业；需要 golden 确认。
4. Psionic 的 `SpellPowerType::Psionic` 与 `P4-PSI-1` 的 coupling 边界。
