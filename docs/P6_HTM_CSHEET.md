# P6 HTM csheet 差分

> 状态：已完成。broad cohort 已包含 HTM csheet case。

## 1. 背景

PCGen 测试套件同时提供 XML 与 HTML 角色卡：

- XML：`code/testsuite/csheets/*.xml`
- HTM：`code/testsuite/csheets/*.htm`

早期只支持 XML。P6-1a 增加 HTM 抽取，用于覆盖更多真实角色。

## 2. 抽取器

`scripts/htm_csheet.py` 通过表格标签抽取：

| 字段 | 抽取方式 |
|---|---|
| hit_points | `Hit Points` 后首个粗体数字 |
| armor_class | `Armour Class` 后首个粗体数字 |
| initiative | `INITIATIVE` 后首个带符号数字 |
| bab | `BASE ATTACK` 后首个数字 |
| save_fortitude/reflex/will | 各豁免行首个粗体数字 |

HTM 不提供 final stats / touch / flat-footed AC，这些字段在差分中跳过。

## 3. cohort 扩展

新增：

- `35e_L7-3(fighter-cleric-dwarf)`
- `35e_L10(fighter-human)`

两者都使用 `35e_L7-3(fighter-cleric-dwarf-std).htm` 风格的 `-std` sheet。

## 4. 自定义装备修复

HTM case 暴露了 `.pcg` 自定义装备解析缺口：

```text
EQUIPNAME:+2 scale mail|...|CUSTOMIZATION:[BASEITEM:Scale Mail|DATA:...EQMOD=MWORKA.PLUS2A.STEEL...]
```

修复：

- `BASEITEM:` → `equipment_base_items`
- `EQMOD=` → `equipment_modifiers`
- `PLUSnA` / `PLUSnS` 增强加值折入 AC 的 armor / shield 分量
- 自定义装备按基础物品解析护甲/盾牌类型

结果：

| case | Rust AC | Java AC |
|---|---:|---:|
| 35e_L7-3 | 21 | 21 |
| 35e_L10 | 28 | 28 |

## 4b. 变体支持（P6-1b）

| 变体 | 支持 | 说明 |
|---|---|---|
| `-std` | ✅ | 服务端渲染表格 |
| `-cmp` | ✅ | 与 `-std` 同结构 |
| `-cmb` | ✅ | JavaScript `CheckArr` / `ATKArr` / `ACObject` / `InitObject` |

`-cmb` 额外提供 touch / flat-footed AC，因此 `35e_L7-3` 使用
`-cmb` 做全字段 Java-gated。

`msrd_Elwood` / `msrd_Ilyana` 的 AC / reflex 差异记录在
`broad_cohort_extended.json` 的 `known_gaps`，每条都是
`{field, owner, issue}`，并由 extended 报告/独立检查约束；它们不进入
主 cohort 的 Java-gated 分母。约束为：

- 与 `java_gated_fields` 互斥；
- 两者并集必须覆盖 Java sheet 实际暴露的字段；
- 默认拒绝新增 gap，必须用 `--write-baseline --accept-known-gaps` 显式接受；
- `--report` 的 `all_field` 会把它计入一致率，`gated` 不计入，
  因此差异不会伪装成通过，也不会消失在报告里。

HTM sheet 不含 `final_stats` / touch / flat 时，这些字段进入
`java_missing`，不从 all-field 分母扣除，不再被误报为分歧。

## 5. 当前边界

- HTM 不提供属性、技能、法术字段；
- `-cmb` / `-sb2` 变体 sheet 格式不同，暂不纳入；
- touch / flat-footed 仍只由 XML case 覆盖。
