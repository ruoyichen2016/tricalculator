# P5 Pathfinder 1e 试运行记录

> 状态：技术试运行可执行；Pathfinder 1e 仍为 deferred，不进入正式门禁。
> 日期基线：P5-2 后。

## 1. 目的

验证当前 PCGen-Rust 是否能够：

1. 导入 PCGen 的 Pathfinder 1e 数据；
2. 解析 Pathfinder `.pcg`；
3. 执行基础 `recalculate`，观察差距与阻塞点。

本试运行不代表 Pathfinder 已纳入产品范围，也不更新正式 coverage baseline。

## 2. 为试运行补的通用导入修复

Pathfinder 数据暴露了三个兼容层问题：

1. `.pcc` include 使用 Windows 路径分隔符，如
   `PCC:@\pathfinder\paizo\...`；
2. 部分 support `.pcc` 只有 `KEY:`，没有 `CAMPAIGN:`；
3. 数据文件条目带内联条件，如
   `CLASS:data/classes.lst|PRECAMPAIGN:1,BOOKTYPE=Core Rules`。

已修复：

- `resolve_path` 将 `\` 归一化为 `/`；
- 允许 `KEY`-only support PCC；
- 数据文件路径在 `|` 处截断，条件保留在 `raw` 中。

这些修复对其它使用 Windows 路径或内联条件的 PCGen 数据同样有效。

## 3. 导入试运行

命令：

```bash
pcgen import-report \
  data/pathfinder/paizo/roleplaying_game/core_rulebook/core_rulebook.pcc \
  --json
```

结果：

| 指标 | 值 |
|---|---:|
| entities | 12,343 |
| tag occurrences | 88,242 |
| lowered effects | 18,577 |
| legacy tags | 14,324 |
| unsupported tags | 654 |
| races | 131 |
| classes | 68 |
| skills | 132 |
| spells | 786 |
| equipment | 969 |
| progressions | 68 |
| abilities | 5,019 |
| ability categories | 175 |

主要 unsupported：

```text
BONUS        564
!PREFACT      67
PREFACT        9
PREMULT        7
PRETEXT        7
```

结论：Pathfinder 数据可以完成语法导入，但存在大量 Pathfinder 特有语义未实现。

## 4. `.pcg` recalculate 试运行

使用 Core Rulebook 对以下 Pathfinder PCG 执行基础 recalculate：

- `pf_Cleric.pcg`
- `pf_Paladin.pcg`
- `pf_Rogue.pcg`
- `pf_goldielocks.pcg`

结果：

| PCG | 可计算 | 主要现象 |
|---|---|---|
| `pf_Cleric` | 是 | 基础 HP/AC/BAB 可输出；caster level 有诊断；大量 `Caster_Level_*` 公式未解析 |
| `pf_Paladin` | 是 | 基础字段可输出；仍有 `SKILL|TYPE=...` unresolved choice |
| `pf_Rogue` | 是 | 基础字段可输出；大量 feat grant prerequisite 诊断为未满足 |
| `pf_goldielocks` | 是 | 可运行但 level / caster level 结果不可信，Pathfinder 专属模型缺失 |

结论：

- 当前可做“接管级”试运行：不崩溃、能导入、能输出基础 snapshot；
- 不能做“结果级”试运行：Pathfinder 数值不应作为正确性验收；
- Pathfinder 专属 CMB/CMD、caster level 公式、archetype、traits、
  favored class 等仍需独立 profile 工作。

## 5. 建议

1. **Pathfinder 维持 deferred。**
2. 不把 Pathfinder 数据加入 DS-3E / DS-35E / DS-PSI coverage gate。
3. 本次通用导入修复可以保留，因为对非 Pathfinder 数据也兼容。
4. 若未来启动 Pathfinder，应先开 `P5-PF-0` 设计 spike：
   - 数据版本；
   - profile 范围；
   - CMB/CMD 模型；
   - 首个 acceptance case；
   - 独立 baseline 与 Java/PCGen 对照。
