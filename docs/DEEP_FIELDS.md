# Deep-Field Aggregate Oracle

> 状态：**L3b-1 ~ L3b-4 全部完成**。7 个代表 case、49 个 Java-gated
> 字段、0 owned known_gaps。

## 1. 覆盖字段

| 字段 | 语义 |
|---|---|
| `equipment_ac_bonus` | armor + shield 合计（规避 PCGen XML 的字段表示差异） |
| `skill_total` | Java `ranks > 0` 的训练技能修正总和 |
| `spell_dc_max` | Rust/Java 共有 spell key 的 DC 最大值 |
| `caster_level_max` | 基础 caster level + `caster_level_bonus` |
| `prepared_count` | Rust `explicit_prepared_count`（仅 `BOOK:Prepared*`）vs Java `memorized_spells`（排除 racial innate） |
| `melee_attack` | BAB + STR + size |
| `ranged_attack` | BAB + DEX + size |
| `grapple_attack` | BAB + STR + grapple size modifier |

每个字段都必须是：

- `gated`：与 Java csheet 精确一致；
- `known_gaps`：带 `{field, owner, issue}` 的已确认差异（当前为空）；
- visibility：当前为空，攻击总额已提升为 gated。

## 2. 代表 case

| case | gated 字段数 |
|---|---:|
| `3e_BarJack` | 6 |
| `3e_SWizSam` | 8 |
| `3e_WizShar` | 8 |
| `3e_BrdJoe` | 8 |
| `3e_FigFae` | 6 |
| `35e_Bard` | 6 |
| `3e_CleElf` | 7 |
| **合计** | **49** |

## 3. L3b-2 ~ L3b-4 修复内容

- **L3b-2 归一化**
  - `equipment_ac_bonus` 合并 armor/shield；
  - `skill_total` 只看 Java `ranks > 0`；
  - `spell_dc_max` 只比较共有 spell key；
  - `caster_level_max` 只在双方都有等级时比较。
- **L3b-3 domain caster level**
  - 识别 active domain `DOMAINGRANTS` 中的 “caster level” 文本；
  - `CharacterSnapshot.caster_level_bonus` 进入 `caster_level_max`；
  - 修正 `3e_CleElf` Law domain +1。
- **L3b-4 prepared / attack totals**
  - `.pcg` 解析 `BOOK:`，`DraftSpell` / `PcgSpell` 新增 `book`；
  - `prepared_spells` 保留原有 spell-effect 激活语义；
  - 新增 `CharacterSnapshot.explicit_prepared_count`，只统计
    `BOOK:Prepared*`，供 deep-field 与 Java `memorized_spells`
    （排除 `racial_innate_memorized`）对齐；
  - `CharacterSnapshot` 新增 `melee_attack` / `ranged_attack` /
    `grapple_attack`，攻击字段从 visibility 提升为 gated。

## 4. 命令

```bash
python3 scripts/deep_field_oracle.py --check --json
python3 scripts/deep_field_oracle.py --report
python3 scripts/deep_field_oracle.py --write-baseline
```

`PCGEN_TESTDATA_DIR` 与 broad oracle 共用同一数据根；门禁已接入
`scripts/check.sh` 与 full-data CI。

## 5. 后续扩展（非本轮）

- skills / spell 逐 key 门禁（当前为聚合标量）；
- 更多 primary case 进入 deep-field cohort；
- MSRD / PF profile 的 depth gate（需产品确认）。
