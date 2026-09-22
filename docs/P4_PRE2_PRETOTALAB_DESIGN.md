# PRETOTALAB 时序设计（P4-PRE-2）

> 状态：已实现（2026-09-21）。Rust 命名为 `TotalBab`（避免 clippy 的 upper-case acronyms 警告），对应 PCGen `PRETOTALAB`。实现见 `pcgen-engine/src/bab.rs`、`Predicate::TotalBab`、`EffectState.total_bab`。
> 目标：在不引入循环求值和不确定行为的前提下，让 `PRETOTALAB` 在
> `grant_allowed` / requirement 求值时拿到稳定的 total BAB。

## 0. 结论

采用 **BAB 独立前置派生状态 + 两阶段求值**：

1. 先计算 `BAB_prereq`：由职业 progression / monster class 公式 /
   gestalt 合并，加上无条件的 `BONUS:COMBAT|BASEAB` 与 `EPICAB` 构成；
2. 把 `BAB_prereq` 作为 `EffectState.total_bab` 传给 predicate 求值；
3. effect closure 结束后，再用**同一个** `compute_bab()` 函数，以完整的
   `effect_state.variables` + `effect_state.external_modifiers` 复算最终 snapshot BAB。

v1 **不做无界不动点迭代**。如果某个修改 BAB 的 modifier 自身被
`PRETOTALAB` gating，则该 modifier 不参与 `BAB_prereq`，但照常参与最终
snapshot BAB，并输出诊断。这样定义是确定性的、可终止的。

## 1. 当前求值时序

`pcgen-engine/src/engine.rs::Engine::evaluate` 当前顺序：

1. `apply_effects(pack, draft)` → `EffectState`（variables / external_modifiers /
   active_abilities）。
2. 解析 race、stats、equipment、AC、gestalt。
3. 计算 BAB / saves：遍历 class levels，使用 `ClassProgress` 或
   `ClassDefinition.base_attack_formula`，公式求值时读取 `effect_state.variables`。
4. 构造 `CharacterSnapshot`，此时 `base_attack_bonus` 只有第 3 步的结果。
5. `apply_external_modifiers(&mut snapshot, &effect_state.external_modifiers)`，
   把 `COMBAT.BASEAB` 等 external modifier 加到 snapshot BAB 上。

因此：

- `apply_effects` 里的 `predicate_holds()` **看不到** 第 3 步的 class BAB；
- 更看不到第 5 步才追加的 `BONUS:COMBAT|BASEAB` modifier；
- `PREVAR*` / variables 也属于 `apply_effects` 内部，存在同样的先后问题。

## 2. PCGen Java 语义

Java `PreTotalABTester.passes()`：

```java
int att = character.baseAttackBonus();
att += character.getTotalBonusTo("COMBAT", "EPICAB");
runningTotal = prereq.getOperator().compare(att, operand);
```

`PlayerCharacter.baseAttackBonus()` 返回 `getTotalBonusTo("COMBAT", "BASEAB")`，
并支持 `COPYMASTERBAB` 特例。

对 Rust 的结论：

- `PRETOTALAB` 的 total BAB = class BAB + `BONUS:COMBAT|BASEAB` modifier；
- `EPICAB` 只用于 `PRETOTALAB`，不进入普通 snapshot BAB；
- Monster (Racial) HD **计入** PRETOTALAB（与 `PREPCLEVEL` 相反）；
- `COPYMASTERBAB` 属于 companion 语义，当前不在 P4-PRE-2 范围。

## 3. Total BAB 的组成

| 组成 | Rust 当前来源 | 当前阶段 |
|---|---|---|
| 普通职业 BAB | `pack.progressions[class].bab(level)` | 第 3 步 |
| Monster / formula BAB | `ClassDefinition.base_attack_formula`（`BONUS:COMBAT\|BASEAB` 被 typed 化） | 第 3 步 |
| Gestalt 合并 BAB | `plan_gestalt` 的 `base_attack_bonus_increment` 按 policy 合并 | 第 3 步 |
| `BONUS:COMBAT\|BASEAB` modifier | `effect_state.external_modifiers`，最终由 `apply_external_modifiers` 加入 | 第 1 / 5 步 |
| `BONUS:COMBAT\|EPICAB` | 当前 snapshot 未接入；只用于 PRETOTALAB | 未支持 |
| `BONUS:COMBAT\|BAB` | Java `baseAttackBonus()` 不读取，因此不属于 total BAB；当前 Rust 也不应用 `COMBAT.BAB` | 不纳入 |
| `COPYMASTERBAB` | 未支持 | out of scope |

**关键约束**：`BAB_prereq` 与最终 `base_attack_bonus` 必须复用同一个
`compute_bab()`，否则会重复计算 monster class formula 或 BASEAB modifier。

## 4. 推荐方案：独立前置派生状态 + 两阶段求值

### 4.1 阶段 A：派生 `BAB_prereq`

新增 `pcgen-engine/src/bab.rs`，导出：

```rust
pub fn compute_bab(
    pack: &CompiledRulePack,
    draft: &CharacterDraft,
    variables: &BTreeMap<String, i32>,
    external_modifiers: &BTreeMap<String, i32>,
) -> i32;

pub fn derive_prereq_bab(pack: &CompiledRulePack, draft: &CharacterDraft) -> i32;
```

`compute_bab()` 负责：

1. 普通职业 progression BAB；
2. monster class `base_attack_formula`；
3. gestalt 合并 BAB；
4. 从 `external_modifiers` 中取 `COMBAT.BASEAB`（与 Java
   `baseAttackBonus()` 一致，不读取 `COMBAT.BAB`）；
5. 对 PRETOTALAB 专用调用额外加 `COMBAT.EPICAB`。

`derive_prereq_bab()` 使用：

- `variables`：先使用空表（v1 限制，见第 8 节）；
- `external_modifiers`：只收 **无 `PRETOTALAB` 依赖** 的 `COMBAT.BASEAB` modifier；
- 不包含被 `PRETOTALAB` gating 的 BAB modifier。

### 4.2 阶段 B：effect closure

`apply_effects(pack, draft)` **保持现有签名**；函数开头调用
`bab::derive_prereq_bab(pack, draft)`，把结果写入 `EffectState`：

```text
EffectState {
    total_bab: BAB_prereq,
    ...
}
```

这样所有调用点自动获得 `PRETOTALAB` 能力，不需要破坏 public API。

`Predicate::TotalBab { min }` 求值：

```rust
Predicate::TotalBab { min } => state.total_bab >= *min
```

这样：

- `grant_allowed()` 在展开 Grant / AUTO 时能校验 `PRETOTALAB`；
- `PREMULT:...` / `Not(...)` 中的 `PRETOTALAB` 也自然可用。

### 4.3 阶段 C：最终 snapshot BAB

`Engine::evaluate` 第 3 步改为：

```rust
let base_attack_bonus = bab::compute_bab(
    &self.pack,
    draft,
    &effect_state.variables,
    &effect_state.external_modifiers,
);
```

Gestalt 分支同样复用 `compute_bab()`，避免两套逻辑漂移。

## 5. 循环求值防护

### 5.1 循环定义

- **写 BAB**：effect target 为 `COMBAT.BASEAB`、`COMBAT.EPICAB`
  （`COMBAT.BAB` 不属于 Java total BAB，列入会误判循环）。
- **读 BAB**：predicate AST 中出现 `TotalBab`（包括 `All/Any/Not/AtLeast` 嵌套）。

若某个“写 BAB”的 effect 的 condition 直接或间接包含 `TotalBab`，则形成
`BAB → PRETOTALAB → BAB` 自引用。

### 5.2 v1 策略

1. `derive_prereq_bab()` 扫描 active entity 的 effects，遇到同时满足
   “写 BAB” 和 “condition 读 BAB” 的 effect 时：
   - 不把它计入 `BAB_prereq`；
   - 记录诊断 `PRETOTALAB_BAB_CYCLE: <source>`。
2. 该 effect 在 effect closure 中照常参与，因此它仍然计入最终 snapshot BAB；
   只是它不能改变 `PRETOTALAB` 对自己的判断。
3. 不进行无界迭代，保证终止和确定性。

### 5.3 为什么不做 v1 全量不动点

- BAB modifier 与变量 modifier 相互依赖时可能出现振荡；
- 当前 engine 已有 16 次固定点循环，但那是 active set / variable 的循环，
  把 BAB 拉进去会扩大搜索空间和调试成本；
- 3e / 35e SRD 的 10 处 `PRETOTALAB` 都是“职业等级产生的 BAB >= N”的
  qualification，不依赖 BAB modifier；v1 足够正确。

如果后续真实数据出现“BAB modifier 被 PRETOTALAB gating”且 Java 结果不同，
再升级为 **有界 refinement**：

```text
BAB0 = derive_prereq_bab()
evaluate effects with BAB0 -> BAB1
if BAB1 != BAB0 and no cycle: evaluate once more with BAB1 -> BAB2
if BAB2 != BAB1: diagnostic + use BAB1
```

最多 1 次 refinement，仍保证终止。

## 6. 数据模型与接口改动

| 位置 | 改动 |
|---|---|
| `pcgen-core::Predicate` | 新增 `TotalBab { min: u32 }` |
| `pcgen-ir::parse_predicate` | 新增 `PRETOTALAB` 解析；`N` → `TotalBab{min:N}` |
| `pcgen-engine::EffectState` | 新增 `total_bab: i32` |
| `pcgen-engine::effects::predicate_holds` | 新增 `TotalBab` 分支 |
| `pcgen-engine/src/bab.rs` | 新增 `compute_bab` / `derive_prereq_bab` |
| `Engine::evaluate` | 阶段 A/B/C 调整，最终 BAB 复用 `compute_bab` |

`!PRETOTALAB:N`：

- Java writer 支持 `< N`（inverted）；
- 当前固定 SRD 没有 `!PRETOTALAB`；
- v1 可先支持 `PRETOTALAB:N`；`!` 的通用处理与 `!PRE*` 一起排期，
  或在 `lower_records` 中对 `!PRETOTALAB` 做局部 `Not` 包装。

## 7. 测试与门禁

### 7.1 单元 / 集成

- lowering：
  - `PRETOTALAB:3` → `TotalBab { min: 3 }`；
  - `PRETOTALAB:3` 嵌入 `PREMULT` 可 lower；
  - `!PRETOTALAB:3`（若本轮支持）→ `Not(TotalBab { min: 3 })`。
- engine：
  - Fighter level 3（BAB=3）满足 `PRETOTALAB:3`；
  - Fighter level 2（BAB=2）不满足；
  - `PRETOTALAB` 计入 monster HD：monster 3 HD 的 BAB 参与判断；
  - Grant / AUTO 中被 PRETOTALAB gating 的 ability 正确阻断。

### 7.2 真实数据与门禁

- 真实 3e SRD：10 处 `PRETOTALAB` 全部 lower；
- `unsupported_tags` 35 → 25；
- `coverage_gate.py`：
  - real 3e SRD 阈值 `unsupported_tags <= 25`；
  - `zero_tags=["PRETOTALAB"]`；
- broad oracle `--check --json` 不变；
- curated Java 9/9 不变；
- archive differential 0 regression / 0 divergence。

### 7.3 完成定义（P4-PRE-2）

- `unsupported_pending_by_tag["PRETOTALAB"] == 0`；
- `unsupported_tags <= 25`；
- `TotalBab` 在 lowering / engine / gate 三层都有测试；
- 文档更新 `ROADMAP.md` / `docs/TAG_WORKLIST.md` / `CHANGELOG.md`。

## 8. 开放问题

| 问题 | v1 处理 | 后续 |
|---|---|---|
| class formula 使用 `var("...")` | 空变量表，默认 0；若公式含 `var(` 输出诊断 | 引入变量求值前置阶段 |
| 条件性 `BONUS:COMBAT\|BASEAB` | 不计入 `BAB_prereq`，只计入最终 BAB | 有界 refinement |
| `COPYMASTERBAB` | out of scope | companion 语义任务 |
| `EPICAB` | 计入 `BAB_prereq`；当前 3e SRD 无样本 | 保持与 Java 对齐 |
| `!PRETOTALAB` | v1 可选；先确认通用 `!PRE*` 方案 | 与 `!PRERACE` / `!PRETYPE` 一起处理 |
| 最终 snapshot 是否显示 EPICAB | 当前不显示；只用于 PRETOTALAB | 如需显示再单独决策 |

## 9. 结论

`PRETOTALAB` 不适合直接在现有 `apply_effects` 里临时算 BAB。
推荐把它建模为 **显式前置派生状态**：由 `compute_bab()` 统一产生，
`derive_prereq_bab()` 为谓词提供不依赖循环的输入，effect closure 后
再用同一函数复算最终 snapshot BAB。这样既满足 Java 语义的主要部分，
又保持确定性、可测试和可终止。
