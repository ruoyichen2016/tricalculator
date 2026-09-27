# P5 KIT runtime 决策启动（设计 spike）

> 状态：决策已定，实现仍 deferred。
> 本文是启动 KIT runtime 的前置设计，评审通过后再开实现任务。

## 1. 结论摘要

- **KIT runtime 是“角色创建期随机展开”**，而当前主业务是重算已有 `.pcg`。
- 因此本阶段**不实现 runtime**，但**冻结启动条件**，避免未来重复评估。
- KIT 声明式 typed 导入（P4-KIT-1）已完成并保持。

## 2. 需要实现的能力

| 能力 | 数据来源 |
|---|---|
| 权重选择 | `OPTION:1,60` |
| 二次查表 | `LOOKUP:Special Ability (B),roll("1d100")` |
| 随机表值 | `VALUES:...` |
| 随机数量 | `QTY:roll("1d6")*1000` |
| 起始预算 | `EQUIPBUY:0` |
| 应用模式 | `APPLY:INSTANT` |
| 嵌套表 | `TABLE:` + `KIT:` |

## 3. 前置条件（启动前必须冻结）

1. **产品确认**：确实需要创建期随机展开。
2. **确定性 RNG**
   - 固定 seed；
   - 跨平台一致（同一 seed 同结果）；
   - seed 由调用方提供，不允许隐式随机。
3. **语义规范**
   - `roll("NdM")` 的 RNG 消耗顺序固定；
   - `LOOKUP` 边界与空表行为；
   - `VALUES` 权重与重复值处理；
   - `EQUIPBUY` 超预算行为；
   - `APPLY:INSTANT` 与 `APPLY:DELAYED` 差异。
4. **Java/PCGen 对照**
   - 至少一个默认起始包（如 `Dwarf Default`）；
   - 至少一个随机宝物表（如 `Minor Armor and Shields`）；
   - 与 Java PCGen 同 seed 输出对照。
5. **独立门禁**
   - `tests/fixtures/kit/*.json` 固定 seed + 期望输出；
   - 不进入现有 `CharacterSnapshot` 非回归门禁；
   - 随机展开单独 CI job。

## 4. 建议实现分期

| 阶段 | 内容 |
|---|---|
| KIT-R1 | 固定 seed RNG + `roll()` + `QTY` |
| KIT-R2 | `OPTION` 权重 + `VALUES` 表 |
| KIT-R3 | 嵌套 `LOOKUP` / `TABLE` / `KIT` |
| KIT-R4 | `EQUIPBUY` + `APPLY` + `.pcg` 回写 |

## 5. 不启动的理由

- 对已有 `.pcg` 重算没有价值；
- 会引入不确定性，污染现有 snapshot 差分门禁；
- 需要独立 fixture 与 Java 对照，成本高；
- 当前更高价值工作在 spell numeric / web GUI / choice 语义。

## 6. 启动条件检查表

- [ ] 产品确认需要随机创建
- [ ] seed 语义冻结
- [ ] `roll` / `LOOKUP` / `VALUES` 语义冻结
- [ ] Java 对照 fixture 就绪
- [ ] 独立门禁与 CI job 就绪

以上全部满足后，才把 KIT-R1 加入 roadmap。
