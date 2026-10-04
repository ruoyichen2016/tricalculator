# PCGen-Rust

PCGen-Rust 是围绕统一 Rule IR、`CharacterDraft` / `CharacterSnapshot`
边界和确定性引擎重构的新架构。

旧实现完整保存在 [`../reference/pcgen-rust-archive`](../reference/pcgen-rust-archive)。

## 架构文档

- [`ARCHITECTURE.md`](ARCHITECTURE.md)
- [`MIGRATION.md`](MIGRATION.md)
- [`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md)
- [`ROADMAP.md`](ROADMAP.md)
- [`docs/TESTING.md`](docs/TESTING.md)
- [`docs/TAG_TRIAGE.md`](docs/TAG_TRIAGE.md)
- [`docs/P4_PRE3_SPELL_TYPE_DESIGN.md`](docs/P4_PRE3_SPELL_TYPE_DESIGN.md)
- [`docs/P4_TEMP1_DESIGN.md`](docs/P4_TEMP1_DESIGN.md)
- [`docs/P4_PSI1_DESIGN.md`](docs/P4_PSI1_DESIGN.md)
- [`docs/P4_DECISIONS.md`](docs/P4_DECISIONS.md)
- [`docs/P5_PATHFINDER_TRIAL.md`](docs/P5_PATHFINDER_TRIAL.md)
- [`docs/P5_PATHFINDER_ASSESSMENT.md`](docs/P5_PATHFINDER_ASSESSMENT.md)
- [`docs/P5_KIT_RUNTIME_DESIGN.md`](docs/P5_KIT_RUNTIME_DESIGN.md)
- [`docs/P6_HTM_CSHEET.md`](docs/P6_HTM_CSHEET.md)
- [`docs/COHORT_MATRIX.md`](docs/COHORT_MATRIX.md)
- [`docs/DEEP_FIELDS.md`](docs/DEEP_FIELDS.md)
- [`docs/P7_PROFILE_SAVE_ENVELOPE_SPIKE.md`](docs/P7_PROFILE_SAVE_ENVELOPE_SPIKE.md)
- [`docs/M9_PERSONAL_BUILDER.md`](docs/M9_PERSONAL_BUILDER.md)
- [`docs/M9_LOOP.md`](docs/M9_LOOP.md)
- [`docs/M9_PACKAGING.md`](docs/M9_PACKAGING.md)
- [`docs/M9_BETA_CHECKLIST.md`](docs/M9_BETA_CHECKLIST.md)
- [`RELEASE_NOTES.md`](RELEASE_NOTES.md)

## 工作区

| crate | 作用 |
|---|---|
| `pcgen-core` | ID、精确有理数、Rule IR、Advancement、Gestalt、图算法、`EntityCategory`/`ChoiceKind` |
| `pcgen-formula` | 公式解析与求值 |
| `pcgen-ir` | 运行时 IR：RawRecord、PatchOperation、typed definitions、`PcgDraft`、CompiledRulePack、降低 pass |
| `pcgen-import` | PCGen 文件格式：`.pcc` 图、`.lst` tokenize、`.pcg` 读写 |
| `pcgen-engine` | `CharacterDraft → Engine → CharacterSnapshot`（只依赖 `pcgen-ir`） |
| `pcgen-builder` | 原生存档 envelope、角色仓库、建卡/升级应用服务 |
| `pcgen-output` | Snapshot text/json 渲染 |
| `pcgen-cli` | `pcgen` 命令行 |
| `pcgen-web` | 本地 HTTP API + 简单 Web 页面（axum） |

## 构建与测试

仓库内测试与 mini 门禁不依赖外部数据：

```bash
CARGO_HOME="$PWD/.cargo-home" cargo test --workspace
python3 scripts/coverage_gate.py --allow-skip
```

真实数据测试与差分门禁需要固定版本的 PCGen 测试数据：

```bash
bash scripts/fetch_testdata.sh      # 固定 commit，写入 <workspace>/raw/pcgen-master
bash scripts/check_testdata.sh      # 校验必需文件与 blob hash
python3 scripts/coverage_gate.py    # 严格模式：跳过真实数据门禁即失败
```

本地 Web GUI（无需外部数据即可启动服务）：

```bash
cargo run -p pcgen-web -- --data-root ../raw/pcgen-master --cohort-root .
# 浏览器打开 http://127.0.0.1:8787
```

API：

```text
GET  /                 # 简单页面
GET  /api/health
GET  /api/curated      # curated cohort 列表 + repeatable abilities
POST /api/recalculate  # { "pcc", "pcg", "overrides": { choices, abilities,
                       #   active_states, class_subclasses, deity, domains } }
POST /api/export       # 同上入参，返回可回写 .pcg 文本 + unresolved + pool usage
GET  /api/characters          # 原生角色列表
                             # 过滤参数：?pcc=…（规则指纹）、?q=…（名称/id）、?class=…、
                             # ?min_level=…、?max_level=…
                             # 页面上有 “only current campaign” 开关与搜索框
                             # 搜索条件会写入地址栏（可分享/刷新保留）
POST /api/characters          # 从 pcc + pcg 保存为原生角色
GET  /api/characters/bundle   # 导出全部原生角色为一个 JSON bundle
POST /api/characters/bundle   # 导入 bundle（覆盖同 id）；不兼容 schema 返回 400，
                             # 兼容但版本/指纹不同时返回 { imported, warnings }
DELETE /api/characters/bundle # 批量删除 { ids: [...] }，返回 { deleted, missing }
GET  /api/characters/bundle/groups  # 按规则指纹分组列出已存角色
POST /api/characters/create   # 按 CreateRequest 建卡并保存
GET  /api/characters/{id}     # 读取原生角色 + snapshot + HTML sheet
DELETE /api/characters/{id}   # 删除原生角色
POST /api/characters/{id}/level-up  # 升级角色（LevelUpRequest）；preview:true 时只预演不保存
                                   # 支持 archetype / 替代等级 / 子职业 / 模板覆盖，以及 lanes[] 多职业
POST /api/characters/{id}/undo     # 还原到上一个保存版本（可连续回退），返回 undo_depth / redo_depth
POST /api/characters/{id}/redo     # 重做最近一次撤销，返回相同详情
GET  /api/characters/{id}/history  # { id, undo_depth, redo_depth, versions[] }
                                   # versions[] 每项含相对当前版本的 summary（等级/职业/能力差异）
POST /api/characters/{id}/history/{n}/restore  # 跳转到第 n 个历史版本（1-based，最旧起）
GET  /api/pack-options?pcc=…  # 种族 / 职业 / 技能 / 法术 / 装备候选
```

建卡向导对购点预算与技能点预算做实时校验（超支高亮）。
页面功能：curated PCG 下拉列表、choice 覆盖、ability pool 编辑、
active state 输入，并展示 snapshot / choices / pools / diagnostics /
HTML 角色卡。角色卡区域提供打印友好的单页布局（`@media print` 只保留 sheet）。
原生角色面板支持导入 / 打开 / 删除，以及升级：选择职业、
可填 HP、并用模板搜索选择器追加生物模板，升级后立即回写存档并刷新快照。

一键开发检查（数据缺失时会明确切换到 no-data smoke）：

```bash
bash scripts/check.sh
```

CI 分为 `no-data`（push / PR）与 `full-data`（nightly / 手动，抓取固定 PCGen
commit 并执行字段级差分）两个 job。细节见 [`docs/TESTING.md`](docs/TESTING.md)。

## M9 打包发布

```bash
python3 scripts/package_release.py --packs rsrd35,srd30 --zip
```

产物：`dist/pcgen-rust-web/`，内含 `run-web.sh` / `run-web.bat`、
`data/packs/rsrd35`、`data/packs/srd30` 与 `characters/`。

## M9 建卡命令（进行中）

```bash
# 3.5e 购点建卡，输出 snapshot
cargo run -p pcgen-cli -- create \
  --pcc ../raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc \
  --name "Hero" --race Human --class Fighter --level 3 --point-buy 25 \
  --stat STR=15 --stat DEX=14 --stat CON=13 --stat INT=12 --stat WIS=10 --stat CHA=8 \
  --skill Climb=4 --ability FEAT=Power Attack --equipment Longsword \
  --template Celestial --starting-gold

# 写入原生存档并升级
cargo run -p pcgen-cli -- create ... --out hero.json
cargo run -p pcgen-cli -- level-up hero.json --pcc <pcc> --class Fighter --hp 6 --out hero2.json

# 查询可用生物模板（--search 按 id/名称过滤；--type-tag 按类型过滤；--format json 输出结构化数据）
cargo run -p pcgen-cli -- list-templates --pcc <pcc> --search celestial
cargo run -p pcgen-cli -- list-templates --pcc <pcc> --type-tag Undead --with-details

# 升级时追加模板（可重复）
cargo run -p pcgen-cli -- level-up hero.json --pcc <pcc> --class Fighter --hp 6 \
  --template Celestial --out hero2.json

# 升级到指定职业等级，并选择 archetype / 替代等级 / 子职业
cargo run -p pcgen-cli -- level-up hero.json --pcc <pcc> --class Cleric --hp 6 \
  --archetype "Cloistered Cleric" --substitution-level "Cleric Substitution ~ level 2" \
  --subclass "Weapon Master" --template Celestial --replace-templates --out hero2.json
```

### 升级预览与多职业

```bash
# 只预演、不写盘：打印升级后的 snapshot
cargo run -p pcgen-cli -- level-up hero.json --pcc <pcc> --class Fighter --hp 6 --preview

# Gestalt / 同一角色等级推进第二个职业 lane（可分别指定 archetype/替代等级/子职业）
cargo run -p pcgen-cli -- level-up hero.json --pcc <pcc> --class Fighter --hp 6 \
  --lane Wizard --lane-hp 3 --lane-subclass Evoker --out hero2.json
```

```text
POST /api/characters/{id}/level-up   # { pcc, request, preview? }
  request.lanes: [{ "class": "...", "hp_gained": 3, "archetype": ..., }]  # 多职业
  preview: true -> 返回 { preview, character, snapshot, delta, html }，不写盘
POST /api/characters/{id}/undo       # 无请求体；还原上一次写入前的版本
```

每次写入前，旧版本会按序号压入 `<characters-root>/.history/<id>/`；
`undo` 弹出最新一份并还原，`redo` 反向重做；任何新的写入都会清空 redo 栈。CLI 等价命令：

```bash
cargo run -p pcgen-cli -- undo hero.json            # 还原
cargo run -p pcgen-cli -- undo hero.json --dry-run  # 只打印还原结果
cargo run -p pcgen-cli -- redo hero.json            # 重做最近一次撤销
cargo run -p pcgen-cli -- history hero.json         # 列出可恢复版本（--format json 结构化）

# 批量导出 / 导入
cargo run -p pcgen-cli -- export-all <characters-root> --out bundle.json
cargo run -p pcgen-cli -- import-bundle bundle.json <characters-root>            # 全部导入
cargo run -p pcgen-cli -- import-bundle bundle.json <dir> --list-groups           # 只看指纹分组
cargo run -p pcgen-cli -- import-bundle bundle.json <dir> --pcc <pcc>             # 只导入匹配指纹的角色
cargo run -p pcgen-cli -- delete <characters-root> --id hero [--id other]
cargo run -p pcgen-cli -- delete <characters-root> --all --dry-run
```

`--template` 可重复；模板按 id 或名称解析（大小写不敏感），未知模板会报错。
`create` 与 `level-up` 都支持 `--template`，`level-up` 追加已应用的模板不会重复；
加 `--replace-templates` 会用本次的模板列表替换旧列表。
`level-up` 还支持 `--archetype` / `--substitution-level` / `--subclass`，为本次升级写入
lane 级选择（替代等级会按目标职业等级校验）。

## PF 1e 作为 3.5e 扩展（变体层，ALPHA）

`extensions/pf35e/` 只补 Pathfinder 的 **archetype / 职业变体**（用 3.5e 语法
重写），以扩展形式挂在 RSRD 之上；基础职业复用 RSRD——PF 在这些职业的
HD / BAB / 豁免上与 3.5e 一致，重写没有意义：

```bash
python3 scripts/package_pf35e_extension.py   # 生成可加载 pcc
PCGEN_BIN=target/debug/pcgen python3 scripts/pf35e_extension_gate.py
# -> pf35e extension gate passed (4 variants layered on the RSRD,
#    64 classes unchanged, variants accepted/rejected by level-up)

# 排查某个变体为什么没生效（目标打不中会显示 UNRESOLVED）：
target/debug/pcgen list-variants --pcc <pcc> --class Fighter
```

验收分四层：**layering**（扩展不改动 RSRD 的职业数、叠加后无 pending tag）、
**parsing**（每个变体都进入 `CompiledRulePack.variants`）、**behaviour**
（`level-up --archetype` 接受合法变体、拒绝未知或跨职业的变体）、
**semantics**（每个 `REPLACES` 目标都真的命中职业特性行——解析成功不等于生效）。
`REPLACES` 的寻址方式、CLI 排查工具与有损项见
[`docs/PF35E_EXTENSION.md`](docs/PF35E_EXTENSION.md)。
无 RSRD 数据时 gate 显式 SKIP（退出码 2）。

## 当前状态

当前实现是架构骨架加核心切片，已经包含：

- 精确有理数与稳定 ID；
- 统一 Effect / Modifier / Predicate IR；
- AdvancementFrame / Gestalt / prestige / archetype / faith / 增量重算；
- 导入层、引擎、输出层、CLI；
- 真实 `.pcc/.lst` source graph 与 lossless Raw AST；
- typed Race/Class/Skill/Spell/Equipment definitions；
- `ClassProgression` 由 import 层生成；
- `import-report` typed counts；
- `.pcg` Draft 读取、`parse-pcg` / `recalculate` / `export` CLI；
- `CharacterSnapshot` 覆盖 HP / BAB / 豁免 / AC / 技能 / 法术 DC /
  caster level / prepared spells / 灵能点字段；
- 9 个 Java XML 差分全部通过；
- archive v1 vs 新 engine snapshot differential；
- 精确 RSRD 灵能点表；
- text / Markdown / CSV / JSON / HTML 输出；
- coverage non-regression gate；
- broad Java differential oracle；
- variant / faith / prestige 诊断；
- 最小 FreeMarker 风格模板；
- EffectApplicator：公式、变量、Grant/AUTO、Choose；
- coverage gate 扩展到 formula/grant/choose/variable；
- external modifiers 接入核心 snapshot；
- 变量依赖图与循环诊断；
- Grant 占位符与内嵌 PRE；
- Deity/Domain typed definitions；
- 执行覆盖率 gate；
- 原生 archetype grants、Faith domain spells/weapon/alignment 校验；
- FreeMarker `#assign` / `#include` / `?size` / `?join` / `?replace` / `?string`；
- broad oracle 区分 data-missing 与真实差异；
- substitution level lowerer；
- FreeMarker `#macro` / `#import as ns`；
- coverage gate 字段级执行覆盖；
- PTS custom class BASEAB/SAVE 公式求值；
- substitution BONUS / ADD:SPELLCASTER variant effect；
- broad oracle per-case PCC 映射；
- coverage gate 字段值级差分；
- class 公式 `var("...")` / `BL=Class`；
- substitution BONUS 真正 lower 为 Effect 并执行；
- FreeMarker macro 默认参数；
- coverage gate 运行 curated Java differential；
- variant Grant / Choose 执行；
- FreeMarker namespace macro；
- Bard 字段值精确门禁；
- macro in #list 精确展开；
- 官方风格 fixture exact golden；
- 真实官方 output sheet smoke（无 FTL 泄漏）；
- Uncanny Dodge flat-footed AC；
- class/class-level 实体激活、无 BASEAB 职业默认 BAB=0、`HP.CURRENTMAX` /
  `TL` modifier 归一化（P4-ENG-1）；
- typed AC 组件与 same-type non-stacking / dodge stacking、默认豁免零值、
  flat-footed 负 DEX、per-level HP minimum、Halfling 种族 trait / ability `.MOD`
  合并、Half-Dragon template stat/AC/HP 接入（P4-BONUS/EQP + P4-RACE/TPL）；
- 装备 `BASEITEM` / `CONTAINS` / `SLOTS` / `CHARGES` typed 导入；
- `EquipSlot` / 冲突诊断 / slot order 排序进入 engine；archive
  `equipment_slot_sort` 意图已有对应测试；
- 统一 `TypedBonus` pipeline：literal 与公式型 `BONUS` 均支持 bonus
  type / stack key，AC / saves / attack / skill 接入；`CharacterSnapshot`
  新增 `attack_bonuses`；
- 法术/领域 `DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` / `ADDSPELLLEVEL` /
  `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK` 与 `PRESPELL` / `PRESPELLSCHOOL`
  接入（P4-SPELL-1 S1）；`spell_effects` 指标/golden 与 snapshot 汇总完成；
- 未训练技能基础展开、`MOVECLONE` 消费、CompanionMod `FOLLOWER:` 行保全；
- P4-CLASS-2 / P4-STAT-2 / P4-FAITH-2 运行时闭环：`SKILLMULT` /
  `LEVELSPERFEAT` / `HITDIE` / `ADDLEVEL` / `EXCLASS` / `EXCHANGELEVEL`、
  `DEFINESTAT` / `STATMOD` / `SIZENUM`、`VALIDFORDEITY` /
  `VALIDFORFOLLOWER` 均已进入 snapshot / 诊断；
- P4-COMP-1 companion runtime：`PRERACE` 条件匹配 + HD / SR / ABILITY /
  creature traits / `BONUS:AC` / `BONUS:STAT`；
- P4-PSI-1/P4-PSI-2 灵能闭环：`power_points` / `manifester_levels` /
  `powers_known` / 灵能 DC / `spell_power_types` 输出，Psion /
  Psychic Warrior / Wilder golden，PRC `ADD:SPELLCASTER|Psionic` stacking；
- P4-EQP-2：`ALTCRITMULT` / `ALTCRITRANGE` / `ALTDAMAGE` / `ALTEQMOD` /
  `ALTTYPE` / `ARMORTYPE` / `BASEQTY` / `CHANGEPROF` / `WEAPONBONUS`
  typed 化，alternate weapon profile 与 `PREWEAPONPROF` 接入 engine；
- P4-SPELL-2：`ADDDOMAINS` / `PROHIBITCOST` / `DONOTADD` / Spell
  `VARIANTS` 消费，条件领域进入 `domain_spells`，`prohibited_costs`
  进入 snapshot；
- P4-ABILITY-1：`AbilityDefinition` / `AbilityCategoryDefinition`、
  `ABILITYLIST` / `POOL` / `STACK` / `MULT` / `CHOICE` typed 化；
  `draft.choices` 优先解析 `CHOOSE`，`unresolved_choices` 输出 diagnostics；
  `ability_lists` / `ability_pools` / `ability_pool_usage` /
  `repeatable_abilities` 进入 snapshot；
- P4-BONUS/EQP 残留：`PREEQUIP*` primary/secondary/both、EquipMod `DR`/`SR`
  trait、`PRECHECKBASE` base-save 谓词；
- P4-TEMP-1 active-state：`TEMPBONUS` / `TEMPVALUE` typed 化，
  `CharacterDraft.active_states` 显式激活临时加值，`CharacterSnapshot`
  输出 `active_states`；
- Race/Template `CreatureTraits` 进入 `CharacterSnapshot`：
  `vision` / `damage_reduction` / `spell_resistance` / `natural_attacks` /
  `reach_feet` / `reach_mult` / `unarmed_damage` / `unarmed_mult`；
  active ClassLevel / Ability / Feat / Equipment 也会汇入 snapshot；
  `creature_trait_records` 作为 metadata catalog；`MOVECLONE`、EquipMod
  `DR`/`SR` 与 CompanionMod 条件行选择均已消费；
- 残余 `BONUS` unsupported 清零；broad oracle 全部字段 23/23（100%），
  known-diverging 0；
- KIT / STARTPACK 声明式 typed 导入（P4-KIT-1，见
  [`docs/P4_KIT0_DESIGN.md`](docs/P4_KIT0_DESIGN.md)）；DS-3E / DS-35E /
  DS-PSI 的 legacy 与 unsupported pending 均为 0，completion 100.0%；
  KIT runtime / Pathfinder 决策见 [`docs/P4_DECISIONS.md`](docs/P4_DECISIONS.md)；
- P5-1：`.pcg` `APPLIEDTO` / `SUBCLASS` 进入 `draft.choices` / `class_subclasses`，
  engine 按 source entity 解析 `CHOOSE`，支持 `CLASS|ALL` / `CLASS|PC`，
  curated PCG 已无 unresolved choices；
- P5-2：`CLASSABILITIESLEVEL` 内嵌 `ADD:...CHOICE`、`SPECIALTIES`、
  `.pcg DEITY` / `DOMAIN` 进入 draft / divine tracks；能力池超支写入 diagnostics；
- P5-4：choice 绑定到 source entity，`%CHOICE%` / `%LIST%` 精确替换；
  `ability_pool_usage.selection_counts` 计数重复选择并诊断非法 MULT；
- P5-5：`CHOSENTEMPLATE` 嵌套选择、`DEITYDOMAINS` catalog 与选定 domain
  分离、信仰 domain 校验、cleric/specialist/template case 选择基线；
- P5-6：broad `choice_state` 覆盖 choice_bindings / ability_pool_usage /
  domain_spells / deity_domains；
- P5-7：prepared/domain spell 的 AC/save/stat/skill `BONUS` 进入
  `applied_spell_effects`；
- 227 个测试（最新数字与门禁状态见 [`ROADMAP.md`](ROADMAP.md)）。

真实 PCGen 数据解析与完整角色计算正在按 [`MIGRATION.md`](MIGRATION.md) 从
archive 迁移。
