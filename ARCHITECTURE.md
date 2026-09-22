# PCGen-Rust 架构

本文档综合 Tri20 草稿、旧 pcgen-rust 的工程实践，以及《Introduction to
Algorithms》中“先形式化问题、再选择数据结构与算法”的原则，定义新的核心架构。

## 1. 核心原则

1. **兼容层与运行时分离**
   - PCGen `.pcc / .lst / .pcg` 语法只存在于 `pcgen-import`。
   - 运行时只消费 `CompiledRulePack`。

2. **输入与派生分离**
   - `CharacterDraft` 只保存用户的持久输入和选择。
   - `CharacterSnapshot` 是完全重算的派生结果，不写回存档。

3. **规则统一为 IR**
   - `Effect`、`Modifier`、`Predicate`、`Provenance` 是唯一规则原语。
   - 不允许在引擎中散落 PCGen tag 分支。

4. **来源与解释是核心数据**
   - 每个派生值都可以携带 `Contribution` / `Explanation`。
   - `.MOD / .COPY / .CLEAR / .FORGET` 以显式补丁操作记录。

5. **精确算术**
   - BAB、豁免、技能半级、速度、重量等使用 `Rational`。
   - 只在显示或规则边界做 floor / ceil / round。

6. **确定性优先**
   - 哈希表只用于查找，不决定规则顺序。
   - 依赖图使用稳定拓扑排序；循环使用 SCC 诊断为错误。

7. **CLRS 作为参考，不是实现清单**
   - 只自己实现标准库没有且领域需要的图算法：稳定拓扑排序、SCC。
   - 排序、查找、堆使用 Rust 标准库。
   - 没有生产调用点的算法不进入运行时 crate。

8. **覆盖度可度量**
   - 导入层输出 lowered / legacy / unsupported 统计。
   - 未支持语义必须显式保留，不允许静默忽略。

## 2. 分层

```text
PCGen .pcc/.lst/.pcg
        │
        ▼
pcgen-import                 pcgen-ir
  .pcc graph / .lst tokenize   SourceRef / RawRecord / PatchOperation
  .pcg drafts                  Raw AST -> lower -> CompiledRulePack
        │                            │
        └──────────► CompiledRulePack ◄──────────┐
                     EntityId + Effect/Modifier/Predicate      │
                     + Provenance + coverage                    │
                            │                                   │
                            ▼                                   │
                     pcgen-engine  ─────────────────────────────┘
  CharacterDraft
    -> activate
    -> advancement frames / Gestalt / prestige / archetype / faith
    -> dependency plan
    -> deterministic evaluation
    -> CharacterSnapshot
        │
        ▼
pcgen-output / pcgen-cli
```

`pcgen-engine` 的生产依赖为 `pcgen-core` / `pcgen-formula` / `pcgen-ir`；
`pcgen-import` 只作为文件格式层，通过 `pcgen-ir` 进入运行时。

## 3. Crate 职责

| crate | 职责 | 禁止 |
|---|---|---|
| `pcgen-core` | ID、Rational、规则 IR、advancement、Gestalt、图算法、`EntityCategory`/`ChoiceKind` 词汇、`tagclass` 覆盖度分类 | 读取 PCGen 文件 |
| `pcgen-formula` | 公式解析与求值 | 读取 PCGen 文件；做角色数值计算 |
| `pcgen-ir` | RawRecord、PatchOperation、typed definitions、`PcgDraft` 数据、`CompiledRulePack`、降低 pass、覆盖度 | 解析 `.pcc/.lst/.pcg`；做角色数值计算 |
| `pcgen-import` | `.pcc` 图解析、`.lst` tokenize、`.pcg` 读写 | 做角色数值计算；被 engine 依赖 |
| `pcgen-engine` | Draft、规则管线、派生快照、增量重算、解释链 | 依赖 `pcgen-import`；理解 PCGen tag |
| `pcgen-output` | Snapshot 渲染 | 规则计算 |
| `pcgen-cli` | 命令行入口 | 规则实现 |

## 4. 关键类型

### 4.1 输入

```rust
CharacterDraft {
    id, name, race_id, campaign,
    stats,
    advancement: Vec<AdvancementFrame>,
    choices,
}
```

`AdvancementFrame` / `AdvancementLane` 支持：

- 普通角色：每级 1 lane；
- Gestalt：每级 N lane；
- `advances`：双进阶职业；
- `archetype` / `substitution_level`；
- `deity` / `domains`。

### 4.2 规则 IR

```rust
Effect {
    phase: EffectPhase,
    condition: Option<Predicate>,
    action: EffectAction,
    source: Provenance,
}

Action = Modify(Modifier) | Grant | GrantTag | SetVariable

Modifier {
    target,
    operation,
    value: Rational,
    bonus_type,
    stack_key,
    condition,
    source,
}
```

### 4.3 派生输出

```rust
CharacterSnapshot {
    draft_id, name, race_id,
    total_level,
    stats,
    hit_points,
    base_attack_bonus,
    saves,
    gestalt: Option<GestaltPlan>,
    explanations,
    diagnostics,
}
```

## 5. 计算流水线

```text
1. validate draft references
2. activate race/class/template/feat/equipment/frames
3. expand grants and choices
4. advancement: HP/BAB/saves/skill points
5. Gestalt merge policy
6. prestige slot cost / archetype / substitution / faith interactions
7. variable dependency graph
8. modifiers and bonus stacking
9. derived snapshot + explanations
10. diagnostics + coverage report
```

阶段 A 的**前置派生**（不依赖 `Effect` 求值，避免循环）目前包括：

- `BAB_prereq`：`TotalBab` 谓词所需的 total BAB（P4-PRE-2）；
- `SpellcastingPrereq`：`PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST`
  所需的 spell power、最高可施法等级、法术位与法术书状态（P4-PRE-3）；
- 生物特征状态：`HANDS` / `MOVE` / `TEMPLATE` 派生的手数、移动模式与模板集合
  （P4-PRE-4），供 `PREHANDS*` / `PREMOVE` / `PRETEMPLATE` 使用。

谓词求值只读这些阶段 A 状态；最终 `CharacterSnapshot` 复用同一份派生结果，
保证「谓词看到的」与「snapshot 输出的」一致。

## 6. 增量重算

- 帧级：`plan_gestalt_incremental` 复用未变化的 frame。
- 依赖级：未来基于 Effect/Modifier 依赖图实现局部重算。
- 缓存只在 session 内生效；规则包 fingerprint 变化时全部失效。

## 7. 算法策略

| 需求 | 方案 |
|---|---|
| `.pcc` include order | 稳定拓扑排序 |
| 循环包含 / 变量循环 | Tarjan SCC + 诊断 |
| 定义查找 | `BTreeMap` / index |
| 排序 | 标准库 `sort_by_key` |
| 搜索 | `partition_point` / `binary_search_by` |
| 堆 | `BinaryHeap` |
| 动态规划 | 只有领域确认需要才引入 |

## 8. 非目标

- 完整 Java CDOM/Facet 事件系统。
- 全部 PCGen 历史 tag 与第三方资料。
- 旧 Swing/JavaFX GUI。
- 把派生值写入角色存档。
- 在运行时继续理解 PCGen 语法。
