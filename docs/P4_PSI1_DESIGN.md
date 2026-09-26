# P4-PSI-1 灵能语义闭环设计

> 状态：首版完成。覆盖灵能威力识别、显能者等级、灵能点、威力 DC 与
> `psionic_effects` 指标，并把 DS-PSI 数据集纳入 coverage gate。

## 1. 数据形态

RSRD 的灵能威力以 `SPELL:` 记录声明，`TYPE:Psionic` 标记灵能属性，
`CLASSES:Psion,Wilder=1` 声明职业等级，`TEMPBONUS:` 声明激活后的临时效果。
PCGen 用 `FACT:SpellType|Psionic` 标记灵能职业（如 `CLASS:Psion`），
部分职业没有 `SPELLSTAT`。

## 2. 识别规则

- `SpellDefinition.is_psionic`：由 `TYPE:Psionic` 决定；作为灵能威力的权威
  标记。
- `ClassDefinition.is_psionic`：由 `TYPE:Psionic` 或
  `FACT:SpellType|Psionic` 决定。
- `import-report.psionic_effects`：统计 `is_psionic` 的 `SPELL` entity 上的
  可执行 effect 数，与 `spell_effects` 口径交叉但对灵能单独暴露。

## 3. 运行时

- `Engine::spellcasting` 跳过 `is_psionic` 职业，避免灵能职业冒出
  `caster_levels`（灵能有独立的显能者等级）。
- `Engine::psionics` 为灵能职业输出 `power_points` 与
  `manifester_levels`；关键属性优先取 `SPELLSTAT`，缺失时按职业默认。
- `CharacterSnapshot.spell_power_types` 包含 `Psionic`；`spell_dcs` 记录
  灵能威力 DC（10 + 威力等级 + 关键属性调整）。
- `PRECHECKBASE:n,Save=v` lower 为 `Predicate::BaseSave`，用于灵能专长的
  基础豁免前提（如 Mental Resistance）。

## 4. 灵能点表

`pcgen-core::psionics` 冻结 RSRD 基础灵能点表（Psion/Wilder、Psychic
Warrior、Psionic Fist、War Mind），加上 `关键属性调整 * 显能者等级 / 2` 的
标准额外灵能点。

## 5. 当前边界

- 威力未知/已知列表尚未进入 snapshot，仍靠 `spells_known` 目标值。
- `FACT:SpellType|Psionic` 的 PRC 灵能等级叠加（`BL=`）未完全建模。
- 灵能威力的 `TEMPBONUS` 通过 P4-TEMP-1 的 active-state 激活，仍需显式
  `active_states` 输入。
