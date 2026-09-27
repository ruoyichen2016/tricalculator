# P5 Pathfinder 1e profile 立项评估

> 状态：评估完成，正式结论为 **deferred / 不立项**。
> 技术试运行见 `docs/P5_PATHFINDER_TRIAL.md`。

## 1. 已具备的能力（试运行证据）

- Pathfinder Core Rulebook 可完成语法导入；
- `.pcg` 可完成基础 `recalculate`；
- 通用 PCG 兼容修复：
  - Windows 路径归一化；
  - `KEY`-only support PCC；
  - `|PRECAMPAIGN` 条件路径。

## 2. 试运行缺口

| 缺口 | 说明 |
|---|---|
| CMB / CMD | Pathfinder 核心战斗数值 |
| caster level 公式 | 大量 `Caster_Level_*` / `charbonusto` 未解析 |
| archetype / traits | 职业与种族替换模型 |
| favored class bonus | 种族/职业奖励 |
| bloodline / mystery | 职业子模型 |
| domain / subdomain | 领域体系差异 |
| Java 对照 | 无固定 Pathfinder csheet cohort |

试运行结果显示：基础字段可输出，但结果级不可信。

## 3. 立项成本估算

| 工作包 | 内容 |
|---|---|
| PF-0 | profile 设计 + 数据源 + acceptance case |
| PF-1 | `GameSystem` / `RulesProfile` 抽象 |
| PF-2 | CMB / CMD / BAB 变体 |
| PF-3 | 职业 / 种族 / traits / archetype |
| PF-4 | 法术 / 领域 / subdomain |
| PF-5 | `.pcg` 读写 + Java 对照 fixture |
| PF-6 | 独立 coverage / CI job |

约等于一个独立 P5 系列，而非 P4 尾巴。

## 4. 结论

- **不立项，维持 deferred。**
- 重新评估条件：
  1. 产品明确要求 Pathfinder 1e；
  2. 定义数据来源与版本；
  3. 定义首个 acceptance case；
  4. 接受独立门禁与独立 CI；
  5. 完成 PF-0 设计 spike。
- 在此之前 Pathfinder 不进入正式 scope，也不影响当前 3e/3.5e 交付。
