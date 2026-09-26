# P4-TEMP-1 active-state 设计

> 状态：已完成首版。覆盖 `TEMPBONUS` / `TEMPVALUE` 的 typed lowering、
> 显式 active state 激活，以及 PC/ANYPC/EQ 三个主要作用域。

## 1. 目标

PCGen 的 `TEMPBONUS` 不是静态常驻加值，而是由法术、能力、装备或状态在
运行时激活的临时效果。引擎不能把它直接混入普通 `BONUS`，否则会在未激活时
污染 snapshot。

## 2. 数据模型

- `CharacterDraft.active_states: Vec<String>`：角色显式激活的临时状态。
- `Predicate::ActiveState { states: Vec<String> }`：临时效果的门控条件；
  只要任一 state 与 draft 中任一 active state 规范化后相等即通过。
- `TemporaryValue { min, max, title }`：`TEMPVALUE` 的 UI 范围元数据，
  存在 `CompiledEntity.temporary_value`。
- `CharacterSnapshot.active_states`：输出已激活状态，便于调试与渲染。

## 3. Lowering 规则

`TEMPBONUS:<scope>|<bonus body>` 会 lower 为普通 `Effect`，但附加
`ActiveState` 条件：

- `PC` / `ANYPC`：只受 active state 门控。
- `EQ`：先解析设备类型限定符，再附加 `Predicate::Equipped`；逗号按 AND、
  分号按 OR 近似处理。
- 原有 `TYPE=` / `STACK=` / `PRE*` 条件继续由 `lower_bonus_effect` 处理。
- state 名称同时使用 record key 与 record name，并去掉 `.MOD` 后缀，
  规范化后存入 `Predicate::ActiveState`。

`TEMPVALUE:MIN=...|MAX=...|TITLE=...` 解析为 `TemporaryValue`，不直接产生
数值 modifier。

## 4. 运行时

`apply_effects` 的 predicate 求值新增 `ActiveState`。只有 active state
匹配时，临时 modifier 才会进入 `EffectState`。

`Spell` / `Skill` 实体没有普通 `draft.spells` / `draft.skills` 激活语义，
因此当它们的 key/name 出现在 `draft.active_states` 时，`initial_entities`
会将其纳入 effect 处理，从而让法术/技能上的 `TEMPBONUS` 可激活。

## 5. 当前边界

- `%CHOICE` 仍按 0 处理，尚未接入 `TEMPVALUE` 的选择值。
- `EQ` 的逗号/分号语义是近似实现，未做严格的装备槽位/堆叠校验。
- `CASTERLEVEL`、`MOVEMULT`、`SIZEMOD` 等临时目标尚未全部进入 snapshot
  字段，目前只按现有 modifier / external modifier 管线处理。
- 未提供 CLI 参数写入 active state；当前通过 JSON draft 或程序化 API 传入。
