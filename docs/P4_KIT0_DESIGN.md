# P4-KIT-0：KIT / STARTPACK 设计 spike

> 状态：设计 spike 已产出，待评审后决定是否进入运行时展开实现。
> 数据来源：DS-35E `_rsrd_basics.pcc` 固定数据。
> 关联任务：`ROADMAP.md` P4-F / P4-KIT-0；`docs/TAG_TRIAGE.md` KIT owner。

## 1. 问题

DS-35E 中 KIT / STARTPACK 相关 pending tag 约占全部 pending 的 72%。
按 category 精确统计如下（数据来自 `compile_pcc` 逐实体探针）：

| tag | KIT / STARTPACK | EQUIPMENT / EQUIPMOD | 说明 |
|---|---:|---:|---|
| `OPTION` | 4,734 | 0 | 随机选择权重，如 `OPTION:1,60` |
| `LOOKUP` | 819 | 0 | 表内二次查表，如 `LOOKUP:Special Ability (B),roll("1d100")` |
| `EQUIPBUY` | 488 | 0 | 起始装备预算，如 `EQUIPBUY:0` |
| `APPLY` | 428 | 0 | 应用模式，如 `APPLY:INSTANT` |
| `QTY` | 276 | 0 | 随机数量，如 `QTY:roll("1d6")*1000` |
| `VALUES` | 130 | 0 | 随机表值定义，如 `VALUES:EQMOD:...|1,20|...` |
| `LOCATION` | 21 | 0（DS-35E） | KIT 侧位置；DS-3E 另有 Equipment `LOCATION` 16 |
| `BASEITEM` | 0 | 140 | 装备基础物品 |
| `CONTAINS` | 0 | 92 | 容器内容 |
| `SLOTS` | 0 | 48 | 装备槽位 |
| `CHARGES` | 0 | 23 | EQUIPMOD 充能 |

KIT / STARTPACK 侧合计 **6,896** 次。`LOCATION` 是明确同名不同义的 tag：

- `KIT` / `STARTPACK` 的 `LOCATION` → **P4-KIT-1**；
- `EQUIPMENT` 的 `LOCATION`（DS-3E 16 次）→ **P4-EQP-1**。

因此实现前必须先落地 `classify(category, tag)`，否则 owner 与 coverage 会互相污染。

## 2. 数据形态

`rsrd_kits.lst`（6,669 行）与 `rsrd_kits_companion.lst` 是主要来源，典型结构包括：

- **默认起始包**：`STARTPACK:Dwarf Default ... EQUIPBUY:0`，后面跟 `RACE` / `CLASS` /
  `STAT` / `SKILL` / `ABILITY` / `GEAR` 等行。
- **随机宝物表**：`STARTPACK:Minor Armor and Shields ... SELECT:roll("1d100")`，
  后续 `KIT:+1 Shield OPTION:1,60` 等。
- **嵌套表**：`TABLE:Special Ability (B) VALUES:...|LOOKUP:...|...`。
- **随机数量/预算**：`QTY:roll("1d6")*1000`、`EQUIPBUY:0`。
- **应用模式**：`APPLY:INSTANT`，与角色创建流程相关。

这些语义明显分成两层：

1. **声明式数据**：KIT/STARTPACK 的名称、类型、条目、条件、权重、表达式字符串；
2. **运行时随机展开**：选择、掷骰、预算、嵌套表解析、GEAR 实例化。

## 3. 范围决策建议

**建议采用 A + 可选 B 的两层方案：**

### A. 声明式导入（立即进入 P4-KIT-1，in scope）

- 新增 `KitDefinition` / `StartPackDefinition` typed 模型；
- 把 KIT/STARTPACK 行解析为结构化条目，保留 `OPTION` / `LOOKUP` / `VALUES` /
  `QTY` / `EQUIPBUY` / `APPLY` 的原始表达式和条件；
- KIT 定义只作为规则包数据存在，不直接改变 `CharacterDraft → Snapshot`；
- 这些 tag 在 coverage 中从 pending 转为 **consumed**，DS-35E completion 随之上升；
- 不要求实现随机选择/掷骰，因此不引入不确定性。

### B. 运行时确定性展开（P4-KIT-2，需产品确认）

若产品明确需要“随机起始包/角色生成”，再实现：

- 固定 seed 的确定性 RNG；
- `OPTION` 权重选择、`QTY` 表达式、`VALUES` 表、嵌套 `LOOKUP`/`KIT`；
- `EQUIPBUY` 预算与 `APPLY:INSTANT` 应用；
- golden fixture 覆盖至少一个默认起始包和一个随机宝物表。

**不建议**让 B 直接阻塞当前核心规则引擎路线；存量 `.pcg` 重算不需要随机展开。

## 4. 建议 typed 模型（草案）

```rust
pub struct KitDefinition {
    pub id: String,
    pub name: String,
    pub kind: KitKind,              // StartPack / Kit / Table
    pub type_tags: Vec<String>,
    pub visible: bool,
    pub apply_mode: Option<String>,
    pub equip_buy: Option<String>,
    pub select_expression: Option<String>,
    pub entries: Vec<KitEntry>,
}

pub struct KitEntry {
    pub target: KitTarget,          // Gear / Race / Class / Stat / Skill / Ability / Template / Kit / Align / Name
    pub key: String,
    pub quantity: Option<String>,   // QTY / RANK / LEVEL 等原始表达式
    pub option: Option<String>,     // OPTION 权重
    pub lookup: Option<String>,     // LOOKUP 二次查表
    pub values: Option<String>,     // VALUES 表定义
    pub conditions: Vec<String>,    // 内嵌 PRExxx / PRE 条件
}
```

表达式（`roll(...)` / `var(...)` / `classlevel(...)`）在 A 阶段保持字符串，避免提前引入运行时语义。

## 5. Coverage 与门禁

- `classify(category, tag)` 必须区分：
  - KIT/STARTPACK 的 `APPLY:INSTANT`、`QTY:roll(...)`、`EQUIPBUY:0`；
  - EQUIPMOD/EQUIPMENT 的 `APPLY:Shield|PREEQUIP`、装备堆叠数量、购买预算。
- P4-KIT-1 验收：
  - DS-35E KIT/STARTPACK 侧的 6,896 次 pending 清零或显式 reclassify；
  - `semantic_completion` 显著上升；
  - broad oracle / archive differential 不变（因为不执行随机展开）；
  - coverage baseline 重冻结一次。
- 若保留 B 为后续任务，则 B 使用独立 fixture 和门禁，不把随机性带进现有 snapshot 门禁。

## 6. 开放问题

1. 产品是否需要随机角色生成，还是只需要 KIT 定义导入与统计？
2. 若需要运行时展开，seed 的来源和可复现性要求是什么？
3. 是否允许把纯随机宝物表标记为 `generation_pending` 独立桶，不计入
   `semantic_completion` 分母？
4. `APPLY` / `QTY` / `EQUIPBUY` 的 category-aware 精确计数需在 P4-KIT-1 前完成。

## 7. 结论与落地状态

P4-KIT-0 的 spike 结论是：**先做声明式 typed 导入，运行时随机展开单独立项**。
这样既能清理 DS-35E 最大 pending 来源，又不会把随机角色生成复杂度引入核心规则引擎。

P4-KIT-1 已按本设计落地：

- `KitDefinition` / `KitEntry` 已进入 `pcgen-core` typed definitions；
- KIT / STARTPACK 的 `OPTION` / `LOOKUP` / `VALUES` / `QTY` / `EQUIPBUY` /
  `APPLY` / `LOCATION` 等 tag 已被 typed import 消费；
- `classify_for_category(category, tag)` 已区分 KIT 与 Equipment 同名 tag；
- DS-35E `semantic_completion` 从 45.5% 提升到 **100.0%**，DS-3E 提升到 **100.0%**。
