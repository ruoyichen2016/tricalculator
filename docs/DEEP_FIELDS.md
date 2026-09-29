# Deep-Field Aggregate Oracle

> 状态：**L3b-1 已完成**。深度字段以独立 config + baseline + owned
> `known_gaps` 的形式进入 full-data 门禁；不扰动 broad oracle 的 10 字段口径。

## 1. 目的

broad oracle 只覆盖 HP / BAB / AC / saves 等 10 个扁平字段。L3b 增加一层
“深度字段”门禁，用来暴露技能、法术、施法者等级与装备 AC 分量的差异：

- `armor_bonus` / `shield_bonus`：装备 AC 分量；
- `skill_total`：Rust 与 Java 共同拥有 key 的技能修正总和（避免 Java 未训练
  技能与 Rust 条目数量不同造成的假差异）；
- `spell_dc_max`：Java csheet 实际暴露的法术 DC 最大值；
- `caster_level_max`：施法者等级最大值；
- `prepared_count`：已准备/已记忆法术计数总和；
- `melee_attack` / `ranged_attack` / `grapple_attack`：**visibility only**；
  Java 已从 `<attack>` 抽取，Rust snapshot 尚未暴露对应字段。

每个字段必须属于以下之一：

- `gated`：必须与 Java 值一致，是真正的 Java-gated 字段；
- `known_gaps`：带 `{field, owner, issue}` 的已确认差异，默认拒绝新增；
- `visibility`：只冻结漂移，不做 Java 断言（当前仅攻击总额）。

## 2. 当前覆盖

配置：`tests/fixtures/differential/deep_field_cohort.json`

- 7 个代表 case；
- **24 个 gated 字段**，分布在 6 个 case：
  `3e_BarJack` / `3e_SWizSam` / `3e_WizShar` / `3e_BrdJoe` /
  `3e_FigFae` / `35e_Bard`；
- **14 条 owned known_gaps**，全部指向 `L3b-2-deep-fields`。

代表性 gated 结果：

| case | 主要 gated 字段 |
|---|---|
| `3e_BrdJoe` | armor / shield / skill_total / spell_dc_max / caster_level_max / prepared_count |
| `3e_FigFae` | armor / shield / skill_total / prepared_count |
| `35e_Bard` | armor / shield / skill_total / prepared_count |
| `3e_SWizSam` / `3e_WizShar` | armor / shield / skill_total / caster_level_max |
| `3e_BarJack` | armor / shield |

## 3. Owned known_gaps（L3b-2 待修）

| field | cases | 典型差异 | owner |
|---|---|---|---|
| `skill_total` | `3e_BarJack`, `3e_CleElf` | ACP / `ACHECK:WEIGHT` 归一化 | `L3b-2-deep-fields` |
| `spell_dc_max` | `3e_BarJack`, `3e_SWizSam`, `3e_WizShar`, `3e_CleElf` | Java 含 spell-like / 非 DC 法术，Rust 侧缺失或语义不同 | `L3b-2-deep-fields` |
| `caster_level_max` | `3e_BarJack`, `3e_CleElf` | SLA caster level / 进阶施法 progression | `L3b-2-deep-fields` |
| `prepared_count` | 4 个 case | Rust 只统计显式 `TIMES`；Java csheet 包含 known/spellbook 条目 | `L3b-2-deep-fields` |
| `armor_bonus` / `shield_bonus` | `3e_CleElf` | 装备 AC 分量归一化 | `L3b-2-deep-fields` |
| `melee_attack` / `ranged_attack` / `grapple_attack` | 全部 7 case | Rust snapshot 未暴露攻击总额，当前仅 visibility | `L3b-2-deep-fields` |

> 这些 gap 是**门禁承认的差异**，不是通过；`--check` 会拒绝新增 gap，
> 并让 stale gap（已修复但仍列为 gap）失败。

## 4. 命令

```bash
python3 scripts/deep_field_oracle.py --check --json   # 门禁
python3 scripts/deep_field_oracle.py --report         # 人类可读差异表
python3 scripts/deep_field_oracle.py --write-baseline # 重新冻结观测值
python3 scripts/deep_field_oracle.py --write-baseline --accept-known-gaps
```

`PCGEN_TESTDATA_DIR` 与 broad oracle 共用同一数据根。

## 5. 接入

- full-data CI：`Deep-field aggregate oracle` step，并覆盖 migrated data root；
- `scripts/check.sh`：有数据时先跑 `cohort_matrix --check` 与
  `deep_field_oracle --check`；
- 主 broad oracle 与 3.0/3.5e acceptance cohort 完全不受影响。

## 6. L3b-2 计划

1. 修 `skill_total`：统一 ACP / `ACHECK:WEIGHT` 语义；
2. 修 `spell_dc_max`：区分 Java spell-like / 非 DC 法术与 prepared spell DC；
3. 修 `caster_level_max`：SLA 与进阶施法 progression；
4. 修 `prepared_count`：区分 known / spellbook / prepared 语义；
5. 修 `3e_CleElf` 装备 AC 分量；
6. 让 Rust snapshot 暴露 melee / ranged / grapple 总额，把攻击字段从
   visibility 提升为 gated。
