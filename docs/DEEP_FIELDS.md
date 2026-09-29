# Deep-Field Aggregate Oracle

> 状态：**L3b-1 + L3b-2 已完成**。残留差异为 owned `known_gaps`，
> 由 L3b-3 继续修复。

## 1. 目的

broad oracle 只覆盖 HP / BAB / AC / saves 等 10 个扁平字段。deep-field
oracle 增加一层聚合字段门禁，暴露技能、法术、施法者等级与装备 AC 分量：

- `equipment_ac_bonus`：armor + shield 合计（PCGen XML 有时把 shield
  折进 armor_bonus，分开比较会产生假差异）；
- `skill_total`：只统计 Java 侧 `ranks > 0` 的训练技能（避免 Java 全技能
  列表与 Rust `.pcg` 条目数量不同造成的假差异）；
- `spell_dc_max`：只比较 Rust / Java 双方共有法术 key 的 DC 最大值；
- `caster_level_max`：只在双方都有施法者等级时比较；
- `prepared_count`：Rust 显式 `TIMES` 计数 vs Java csheet 计数；
- `melee_attack` / `ranged_attack` / `grapple_attack`：**visibility only**；
  Java 已从 `<attack>` 抽取，Rust snapshot 尚未暴露对应总额。

每个 comparable 字段必须属于：

- `gated`：必须与 Java 一致；
- `known_gaps`：带 `{field, owner, issue}` 的已确认差异，默认拒绝新增；
- `visibility`：只冻结漂移，不做 Java 断言。

## 2. 当前覆盖（L3b-2 后）

配置：`tests/fixtures/differential/deep_field_cohort.json`

- 7 个代表 case；
- **23 个 Java-gated 字段**；
- **5 条 owned known_gaps**。

| case | gated 字段 |
|---|---|
| `3e_BrdJoe` | equipment_ac_bonus / skill_total / spell_dc_max / caster_level_max / prepared_count |
| `3e_SWizSam` | equipment_ac_bonus / skill_total / spell_dc_max / caster_level_max |
| `3e_WizShar` | equipment_ac_bonus / skill_total / spell_dc_max / caster_level_max |
| `3e_FigFae` | equipment_ac_bonus / skill_total / prepared_count |
| `35e_Bard` | equipment_ac_bonus / skill_total / prepared_count |
| `3e_BarJack` | equipment_ac_bonus / skill_total |
| `3e_CleElf` | equipment_ac_bonus / skill_total |

L3b-2 修复的差异：

- 装备 AC：合并 armor+shield 后 `3e_CleElf` 两侧均为 7；
- 技能总额：只看 Java 训练技能后 `3e_BarJack`（22/22）与
  `3e_CleElf`（12/12）一致；
- 法术 DC：只比较共有 spell key 后 `3e_SWizSam` / `3e_WizShar` 一致；
- 施法者等级：Java 为空（纯 SLA / 输出 sheet 未暴露）时不再强行比较。

## 3. Owned known_gaps（L3b-3）

| field | cases | 现象 | owner |
|---|---|---|---|
| `prepared_count` | `3e_BarJack` / `3e_SWizSam` / `3e_WizShar` / `3e_CleElf` | Rust 统计显式 `TIMES`；Java csheet 的 known / memorized 语义不同 | `L3b-3-prepared-semantics` |
| `caster_level_max` | `3e_CleElf` | Law domain `DOMAINGRANTS` +1 caster level | `L3b-3-domain-caster-level` |
| `melee/ranged/grapple` | 全部 7 case | Rust snapshot 未暴露攻击总额（当前 visibility） | `L3b-3-attack-totals` |

> `--check` 会拒绝新增 gap，并让 stale gap（已修复但仍列为 gap）失败。

## 4. 命令

```bash
python3 scripts/deep_field_oracle.py --check --json
python3 scripts/deep_field_oracle.py --report
python3 scripts/deep_field_oracle.py --write-baseline
python3 scripts/deep_field_oracle.py --write-baseline --accept-known-gaps
```

`PCGEN_TESTDATA_DIR` 与 broad oracle 共用同一数据根。

## 5. 接入

- full-data CI：`Deep-field aggregate oracle` step，并覆盖 migrated data root；
- `scripts/check.sh`：有数据时先跑 `cohort_matrix --check` 与
  `deep_field_oracle --check`；
- broad oracle 与 primary acceptance cohort 不受影响。

## 6. L3b-3 计划

1. 统一 known / memorized / prepared 语义，消掉 4 条 `prepared_count` gap；
2. 建模 Law domain `DOMAINGRANTS` 的 +1 caster level，消掉
   `3e_CleElf::caster_level_max`；
3. 在 `CharacterSnapshot` 暴露 melee / ranged / grapple 总额，
   把攻击字段从 visibility 提升为 gated；
4. 每修一项，将对应 known_gap 提升为 gated，重跑 `check.sh`。
