## Unreleased

### 扩展：扫描新增两条不变式，并把"孤儿行"的假设变成已核对的事实

在既有游标劫持检查之上扩展：

- **孤儿行细分**：等级行游标为空时区分三种情形——
  - `ORPHAN (.MOD file)`：文件带 `.MOD` 职业行，基础职业在别处（正常）；
  - `UNATTACHED`：文件**完全没有**职业声明却有等级行（真缺陷）；
  - `OUT-OF-ORDER`：文件**有**职业头但等级行出现在其之前（顺序问题）。
  实测三者分别为 **24 / 0 / 0**；
- **跨文件核对（新增 `SCAN2`）**：每个 `.MOD` 职业行的基础职业必须在**同一资料树**
  内有声明。此前"孤儿行正常"只是**假设**，现在被核对：
  35e / 3e / pathfinder 分别 720/534/292 个职业头、132/34/35 个 `.MOD` 目标，
  **悬空均为 0**；
- 该跨文件检查**抓到过一个问题并证明是我扫描的缺陷**：3e 报 `rogue_(cs)` 悬空，
  实为 `CLASS:Rogue.COPY=Rogue (CS)` 声明（`.COPY=` 被我跳过）。修正后归零——
  记录在此，因为"扫描报出问题"与"数据有问题"不是一回事；
- 扫描改为复用 `pcgen_import::is_class_header_row` **同一份**判定
  （该谓词已提升为公开 API），不再在测试里重写一遍逻辑。


### 新增：全库导入健全性扫描（`tree_scan`）

RSRD 验收集很小，`SUBCLASS:` / `SUBSTITUTIONCLASS:` 这类游标劫持 bug 能在其中
长期潜伏。新增一个诊断扫描，走遍 `data/35e`、`data/3e`、`data/pathfinder`
共 1,296 + 918 + 2,939 个 `.lst`（其中 262 个含等级行），用**真实 importer**
（`lower_records`）检查一条自洽不变式：

> `lower_records` 把职业等级实体命名为 `<职业键>#<等级>`，因此该职业键必须是
> 同一 pack 声明过的职业。否则说明游标停在了不该停的地方。

**结果**：

| 分类 | 数量 | 说明 |
|---|---|---|
| **游标劫持（真缺陷）** | **0** | 修复后全库为零 |
| 孤儿等级行（本文件无职业头） | 24（5 个不同键 / 8 个文件） | 均为 `.MOD` 文件，职业头在另一文件；真实 pack 中会先加载基础文件，属正常 |

**扫描有效性已验证**：临时撤掉修复后重跑，报告 **697 个劫持键 / 208 个不同键 /
26 个文件**（含 RSRD 自身）；恢复修复后归零。说明扫描确实能发现该类缺陷，而非
恒返回零。

扫描以 `#[ignore]` 标记（全库约 103 秒），按需运行：

```text
cargo test -p pcgen-engine --test tree_scan -- --ignored --nocapture
```


### 修复：消除"同一判定写两份"的隐患（`SUBSTITUTIONCLASS:` 同类 bug）

上一轮发现 Wizard 特性全失效的根因是**职业头判定在两处各写一份、只修了一份**。
本轮针对这一类隐患做了排查与收敛：

- **新增 `is_class_header_row`，两处共用**：`rulepack.rs` 与 `typed.rs` 各自的
  职业头判定合并为一个函数，杜绝再次漂移。两遍扫描在 `.MOD` 上的策略差异
  **是有意的**（`rulepack` 不把 `.MOD` 当职业头；typed 保留无 base 的游离 `.MOD`
  以免静默丢源），因此该差异保留在调用方，并写进注释；
- **修复同类的 `SUBSTITUTIONCLASS:` bug（潜在，6+ 个第三方文件受影响）**：
  `rulepack.rs` 同样没有排除 `SUBSTITUTIONCLASS:` / `SUBSTITUTIONLEVEL:` 行，
  这些行会把 `current_class` 改成 `SUBSTITUTIONCLASS:...`，其后的等级行被命名成
  `SUBSTITUTIONCLASS:...#1`，于是该职业等级行永远匹配不上。RSRD 不含此类行
  （所以验收集看不出来），但 3.5e 第三方资料有 6 个文件使用，Pathfinder 有 1 个。
  实测修复前键为 `SUBSTITUTIONCLASS:Lesser Elemental Theurge Option#1`；
- **合并重复的 ACF 判定**：`is_acf_record` 与 `is_variant_record` 尾部是同一段
  逻辑，现由前者作为唯一定义，后者调用它；
- 新增回归测试 `substitution_rows_do_not_move_the_class_cursor`
  （已验证：去掉修复即失败，报错信息正是上面那个键）。

> 排查结论：`current_*` 游标仅 `rulepack.rs` / `typed.rs` 两处（均已修）；
> 无同名重复谓词；其余 `"ACF"` 字面量（如 `applies_to` 的池名判定）问的是
> 另一个问题，非重复实现。


### 修复：Wizard 的全部职业特性从未激活（`SUBCLASSLEVEL:` 行被当作职业头）

`lower_records` 里有**两份**职业头判定逻辑，此前只修了 `typed.rs` 那份。另一份
（`rulepack.rs`）仍把 `SUBCLASS:` / `SUBCLASSLEVEL:` 行当成职业头，于是
`current_class` 被改成 `SUBCLASSLEVEL:1`，其后的等级行实体被命名为
`SUBCLASSLEVEL:1#1` 而不是 `wizard#1`。

引擎按 `<class>#<level>` 匹配职业等级来激活该级的职业特性，因此
**Wizard 的全部职业特性从未激活**——法术、魔宠、抄写卷轴、法术书、武器护甲熟练
全部缺失。实测：

- 修复前 Wizard 1 的 `active_abilities` 只有 `LANG|Common` / `defines_required`；
- 修复后为 6 项：`wizard_~_familiar`、`wizard_~_spells`、`wizard_~_scribe_scroll`、
  `wizard_~_spellbooks`、`wizard_~_bonus_languages`、
  `wizard_~_weapon_and_armor_proficiency`。

为什么之前没发现：`class.level_abilities`（`typed.rs` 路径）**是正确的**，只有实体
键（`rulepack.rs` 路径）错。只检查前者的测试看不出问题。新增回归测试
`level_row_entities_are_keyed_by_their_class` 直接断言实体键。

### 特性：反射闪避叠加与魔宠等级叠加（SRD 剩余两个多职业特例）

- **反射闪避**：官方"When a barbarian/rogue would gain uncanny dodge a second time
  (for her second class), she instead gains improved uncanny dodge, if she does not
  already have it. Her barbarian and rogue levels stack to determine the rogue level
  an attacker needs to flank her." 数据只声明各职业自己几级给这两种能力，**没有**
  表达"第二次转成精通"，故在引擎实现。实测 Barbarian 2 / Rogue 4 → 精通反射闪避
  （两职业均未达各自精通等级，只可能来自转换），叠加等级 6；Barbarian 5 / Rogue 8
  → 13；
- **魔宠**：官方"A sorcerer/wizard stacks his sorcerer and wizard levels to determine
  the familiar's natural armor, Intelligence score, and special abilities."。数据用
  `FOLLOWER:FamiliarLVL=<阈值>` 表达，`FamiliarLVL` 由 `SorcererLVL` + `WizardLVL`
  累加。引擎改为**从该变量推导魔宠主人等级**，实测 Sorcerer 3 / Wizard 3 →
  `FamiliarLVL` 6，魔宠 `master_level` 取 6 而非调用方传入值；
- 新增 `snapshot.uncanny_dodge` 与 `tests/companion_stacking_rules.rs`（7 用例）。

> 这两项一开始都测不过，但**根因不是叠加逻辑**——是上面那个 Wizard 特性未激活的
> bug。修好实体键之后叠加逻辑立即通过。记录在此以免日后误判。


### 修复：多职业下 `CL` 恒为 0，以及 `BONUS:VAR` 的逗号列表未拆分

两个 bug 都会**静默**产生错误数值，且都影响面很广。

- **多职业下 `CL` 恒为 0**：`CL` 是 PCGen 的"职业等级"，数据用
  `BONUS:VAR|ClericLVL|CL` 把职业等级写进变量。引擎此前**只在角色只有一个职业时**
  替换 `CL`，多职业时 `CL` 原样留在公式里被求值为 **0** —— 于是
  **所有 `<职业>LVL` 变量归零**，连带影响驱散等级、战士奖励专长数等一切依赖它的
  能力（实测 Cleric 3 / Paladin 6 的 `ClericLVL` 与 `PaladinLVL` 均为 0）。
  现按**效果来源记录所属的职业**解析 `CL`；
- **`BONUS:VAR` 的逗号列表未拆分**：`BONUS:VAR|TurnLevelBase,TurnDamagePlusBase|ClericLVL`
  本应给两个变量赋值，引擎把 `TurnLevelBase,TurnDamagePlusBase` 当作**一个变量名**，
  真变量停留在 `DEFINE` 的 0，并凭空产生一个名字含逗号的变量。修复后
  `TurnLevelBase` 0 → 3、`TurnDamagePlusBase` 3 → 5
  （= 牧师等级 + 魅力调整，符合官方"2d6 + cleric level + Cha"）。

### 特性：驱散/斥退亡灵（含多职业叠加）

- **驱散等级**由数据正确表达官方两条规则：
  "paladins turn undead as if they were clerics of three levels lower" 与
  "her effective turning level is her cleric level plus her paladin level minus 3"。
  实测 Cleric 3 / Paladin 6 → **6**（若取最大值则为 3，是错的）；
  Paladin 3 → 无该能力，Paladin 4 → 1；
- **驱散 vs 斥退**按官方**只看阵营、不看神祇身份**判定：善良 → 驱散，
  邪恶 → 斥退，中立 → 需按神祇阵营；
- 新增 `snapshot.turning`：`level`、`mode`、`times_per_day`（3 + 魅力调整）、
  `damage_bonus`（职业等级 + 魅力调整）、`note`；
- 新增 `crates/pcgen-engine/tests/turning_rules.rs`（10 个用例，逐条引用官方文本）。

> **已知边界**：中立体例牧师的驱散/斥退取决于**神祇阵营**，而 **RSRD 不含神祇**
> （属 WotC 产品标识；`rsrd_deities.lst` 只有一条 `None` 记录，其
> `PresetUndeadChoiceByDeity` 机制 10=驱散 / 20=斥退 被注释掉）。引擎对此返回
> `mode: "choice"` 并附原因，**不猜**；用户经 `data/packs/custom/curated/`
> 补充带阵营的神祇数据后即可完整判定。


### 修复：实现 4 条只写在 SRD 文字里的规则（技能上限、跨职半点制、领域法术位）

以官方 SRD 原文（<https://www.d20srd.org/>）为裁判审计引擎。BAB/豁免/领域法术表
与官方**逐格相符**，但有 4 条规则**只写在文字里**（表格看不出），此前完全未实现：

- **技能最大 rank**（官方："Your maximum rank in a class skill is your
  character level + 3"）：此前可任意超上限，Fighter 1 级能灌 `Climb=8`
  （上限应为 4）；
- **跨职技能上限**（"one-half of this number, do not round up or down"）：
  1 级跨职上限 2.0，此前同样不受限；
- **跨职半点制**（"Each skill point you spend on a cross-class skill gets your
  character ½ rank"）：此前按 **1:1** 计算，导致跨职技能的实际花费被**低估一半**
  ——例如 2 点跨职 rank 实占 4 点预算，旧逻辑只算 2 点；
- **Cleric 领域法术位**（官方表格 Spells per Day 的 `+1`，文字："gains one
  domain spell slot for each spell level she can cast"）：领域法术**列表**
  一直正确，但 `spell_slots` 缺 `+1`；现 Cleric 1 为 `3 / 1+1`。

实现要点：

- 新增 `pcgen_core::skill_rules`，**把官方原文与出处 URL 写在代码里**，
  使"文字规则"成为一等公民而非注释；
- 上限用**角色等级**（非职业等级），并按官方"若某技能是**任一**职业的本职技能，
  上限即为角色等级+3"做**跨职业并集**；
- 新增 `BuilderError::SkillRankExceeded`，错误信息直接引用规则出处；
- 领域法术位只在**有领域**时添加，且**不含 0 环**；无领域与非法师职业不受影响。

**文档虚报修正**：`docs/M9_PERSONAL_BUILDER.md` 早已把"跨职业技能、最大 rank 校验"
列为已完成，但代码中并无实现（全仓搜索 `max_rank` / `cross_class` 零命中）。
本轮同步修正该文档，并在 ROADMAP 记录教训——**能力声明以测试为准，不以文档为准**。

测试：`crates/pcgen-engine/tests/srd_text_rules.rs`（3 个文字规则测试）、
`pcgen-builder` 新增 3 个技能规则测试（含"旧 1:1 逻辑会通过"的反例）。


### 修复：`list-variants` 的锚点解析漏了能力记录，且未命中的原因不可见

- **修复解析范围**：Bard 的 Bardic Music 子特性
  （`TYPE.BardInspireCourage` 等）挂在 `CATEGORY=Special Ability|Bard ~
  Bardic Music.MOD` 的 `ABILITY:` 标签上，而 `.MOD` 在 lowering 时已合并进
  基础实体（`patch_operations` 为 0），这些锚点不进 `CompiledRulePack`。
  解析器此前只查 `pack.classes` / `pack.progressions`，把它们误报为无法解析。
  现在同时扫描**未经 patch 的原始记录**，且只取带
  `CATEGORY=ACF,TYPE.` / `TYPE.Archetype` 守卫的行（无守卫的 `ABILITY:` 不是
  锚点，其尾部谓词片段曾污染锚点列表）；
- 实测 `Unearthed Arcana`：未解析目标 **26 → 18**，`Animal Companion Bard`
  的 5 个锚点全部命中；
- **输出改为列出该职业实际开放的锚点**，而不是猜"近似名字"：
  `MISSING(TYPE=wizardbonusfeats) [class offers: WizardBonusFeat ...]`。
  设计过程中我试过编辑距离、子串包含、CamelCase 词重叠三种相似度启发式，
  都不稳定（`MonkAC` 与 `MonkBonusAC` 中间插入了一整个词，编辑距离 5；
  词重叠又会把 `RangerCombatStyle` 误配给 `RangerImprovedCombatStyle`），
  最终选择**不做相似度判断、直接给事实**；
- **UA 未命中目标的分类（10 个不同目标 / 18 次出现，逐条对照 RSRD 核实）**：
  - 拼写差异 2 个：`WizardBonusFeats` vs RSRD `WizardBonusFeat`、
    `MonkBonusAC` vs `MonkAC`；
  - **RSRD 未开放该锚点 8 个**：`ClericTurnUndead`（RSRD 对 Cleric 开放的
    ACF 锚点为 **0**）、`RogueSneakAttack`（Rogue 仅 `RogueProficiencies`）、
    `DruidArmorProf` / `DruidShieldProf`、`BarbarianImprovedUncannyDodge`
    （被 8 个图腾变体共用）、`RangerImprovedCombatStyle` /
    `RangerCombatStyleMastery`、`BardVariant`（UA 内部标记）；
  - 这些**不是解析器缺陷**，是数据集之间的差异。不做静默近似——把未命中
    "修好"等于替数据编造语义。

### 特性：识别 3.5e 原生 ACF（`CATEGORY:ACF`）

- **PF 的 archetype 与 3.5e 的 ACF 是同一机制**。此前只识别
  `TYPE:Archetype*` / `TYPE:Substitution*`，3.5e 原生 ACF 记录被当作普通
  能力处理，**完全不产生变体**——整套 3.5e ACF 机制（`Unearthed Arcana`、
  `Complete` 系列等）在引擎里不可见；
- `is_variant_record` 增加 `CATEGORY:ACF` 识别；
- 新增 `apply_acf_conventions`：ACF 用约定而非专用标签表达变体语义，
  逐项解析——
  - 所属职业取自 `PREVARGTEQ:<Class>LVL,<n>`（最可靠）→ `SORTKEY` →
    `KEY:` 前缀。**顺序很重要**：`KEY:Totem Barbarian ~ Ape` 的前缀是组名，
    而特性锚点按职业命名（`BarbarianUncannyDodge`），用组名会一条都匹配不上；
  - `TYPE:ACF.<Class><Feature>` 的特性锚点转为 `REPLACES:TYPE=<tag>`，
    与 RSRD 职业行的 `!PREABILITY:1,CATEGORY=ACF,TYPE.X` 守卫**同名对齐**；
  - `ABILITY:...|PREVARGTEQ:<Class>LVL,<n>` 转为等级化 `GRANTS`；
- **修复** `GrantedFeature::parse` 只接受 `Class=level`、遇到裸 `PRECLASS:1,5`
  会静默丢弃等级的问题（此前生成的守卫格式与解析器不匹配，实测等级全为
  `None`）；
- 实测：`Unearthed Arcana` 叠加 RSRD 后产出 **28 个变体**，
  `list-variants` 显示正确职业；`Fighter ~ Sneak Attack` 端到端验证会移除
  `Fighter ~ Bonus Feats` 并追加 `Sneak Attack`；
- 新增回归测试 `crates/pcgen-ir/tests/acf_variants.rs`（5 case）；
- `docs/PF35E_EXTENSION.md` 新增第 5 节说明两者的对应关系，并把「有损之处」
  拆为 **A 类（PF 与 3.5e 的规则差异）** 与 **B 类（引擎实现深度）**——
  原先把「按等级切片的部分替换」写成 PF 专有差异是分类错误，3.5e 的 ACF
  有同样需求；
- `PRODUCT_SCOPE.md` / `ROADMAP.md` 记录该能力，并明确 UA 等资料
  **可用但尚未纳入验收 cohort**（无差分基线）。

### 许可：补齐 `LICENSE` / `NOTICE` 与数据包许可文件 `LICENSE` / `NOTICE` 与数据包许可文件

- 新增 `LICENSE`（LGPL-2.1 全文，与 `Cargo.toml` 的 `license = "LGPL-2.1"`
  一致，也与上游 PCGen 代码许可一致）；
- 新增 `NOTICE.md`，明确本仓库三类许可**不可混为一谈**：
  `crates/`+`scripts/`+文档为 LGPL-2.1；`raw/` 与 `data/packs/` 为
  OGL 1.0a + PCGen 数据条款；`extensions/pf35e/`（含 `.lst` 数据）为独立
  重写、依 Paizo OGL / Community Use；并附 OGL §15 要求保留的署名片段
  （逐条与 RSRD `OGL.txt` 校对）与商标声明；
- **修复数据包不带许可文本**：`package_datapack.py` 此前只在 `pack.json`
  里写一个 `license` 字符串，包内没有任何许可条文。现在每个包根目录会生成
  `NOTICE.md`（含署名与商标），并指向该包数据树内实际存在的 `OGL.txt`
  路径（rsrd35 / srd30 各自不同，已逐一核对）；
- `scripts/test_package.py` 新增许可检查：`LICENSE`/`NOTICE.md` 必须存在、
  `LICENSE` 必须是 LGPL-2.1 正文、`Cargo.toml` 的 license 必须与之一致、
  打包脚本必须真的写出 NOTICE 且引用的 OGL 路径按包正确（已用变异测试
  验证这两类检查会失败）；
- `README.md` 增加「许可」一节；`docs/M9_LOOP.md` 不再把「无 LICENSE 文件」
  列为已知风险；`docs/M9_PACKAGING.md` 说明数据包许可文件。

### 特性：`GRANTS` 支持职业等级

- 变体追加的特性现在可以声明获得等级，写法沿用 PCGen 在真实 archetype 的
  `ABILITY:` 行上使用的 `PRECLASS` 守卫：
  `GRANTS:Weapon Training|PRECLASS:1,Fighter=5`；无守卫即 1 级可用
  （等同 PCGen 对裸 `ABILITY` 标签的处理）；
- `pcgen-core` 新增 `GrantedFeature`（`name` / `level` / `class_id`，
  `applies_at(level)`）与 `VariantDefinition::{grant_entries, grants_at_level,
  apply_to_features_at_level}`；`apply_to_features` 保留为"等级未知"时的
  等级无关入口；
- 引擎 `features_for_class` 改为按 lane 的当前职业等级过滤：1 级 Weapon
  Master 不再提前列出 `Weapon Training`（PF 是 5 级）；`REPLACES` 不受等级
  影响，被替换的特性全程消失；
- **修复导入分片**：`GRANTS` 值此前按 `,` 与 `|` 一并切分，会把
  `PRECLASS:1,Fighter=5` 打散成 `PRECLASS:1`、`Fighter=5`。现在
  `split_grant_entries` 会识别守卫片段并与之合并，同时保持 `A|B` 这种
  无守卫的并列写法可用；
- 扩展数据的等级取 PF Core Rulebook 值：Weapon Training@5 / Weapon Guard@9、
  Armor Training@3 / Armor Mastery@19、Arcane School@1 / Arcane Bond@1；
- `list-variants` 文本与 JSON 都输出等级（`Weapon Training@5` /
  `grant_entries[].level`）；
- gate 新增一层：断言已知的 PF 等级值，等级回归到"一律 1 级"会直接 FAIL
  （已用变异测试验证）；
- 新增 4 个等级门控回归测试
  （`crates/pcgen-engine/tests/variant_grant_levels.rs`）；
- 文档：`docs/PF35E_EXTENSION.md` 的等级有损项已移除（改为"已支持"），
  另将"archetype 的等级化**替换**"单独列明为仍存在的有损项。

### 修复：`SUBCLASS:` 行被当成职业头，导致特性行整批丢失

- **修复** `build_typed_definitions` 的 `current_class` 游标：`SUBCLASS:` /
  `SUBCLASSLEVEL:` 行（如 RSRD 的 8 个法师学派）描述的是**当前职业的专精**，
  不是新职业。此前它们落进"类头"分支，把 `current_class` 改写成
  `SUBCLASS:Abjurer` 之类，于是**紧随其后的等级行全部挂到了伪职业上**；
- 实测影响：RSRD `Wizard` 特性行 **0 → 7**、`Dragon Disciple` **0 → 9**；
  Fighter / Cleric 因等级行在 `SUBCLASS:` 之前而侥幸未受影响；
  `pack.classes` 里的伪职业（`SUBCLASS:*` / `SUBCLASSLEVEL:*`）已消失；
- 数值门禁此前全绿是因为 Wizard 的 BAB/豁免来自 `CLASS:` 头行的 `BONUS:`
  公式，不依赖特性行——特性列表从未被断言过；
- 新增回归测试 `crates/pcgen-ir/tests/class_specialty_rows.rs`（3 case，含
  "无专精行的职业不受影响"反例）；修正两个把旧行为写进断言的测试
  （`stage2_equipment_spell`、`stage3_choice_pool`）；
- `coverage_gate.py` 的 DS-3E `classes` / `progressions` 下界 45 → 43，
  因为其中 2 个本来就是 `SUBCLASS:` 伪职业（真实职业清单未变）。

### 修复：`REPLACES` 从未真正生效（变体只加不减）

- **根因**：引擎里特性是结构化行
  `Fighter Class Feature|AUTOMATIC|Fighter ~ Bonus Feats|!PREABILITY:1,...`，
  而 `apply_to_features` 用 `eq_ignore_ascii_case` 做**全串相等**，与
  `REPLACES:Fighter Bonus Feat` 这类字面名永远匹配不上：4 个变体实测
  `removed=0`；
- `pcgen_core` 新增共享匹配器：`feature_name`（取最后一个非谓词 `|` 段、
  剥离 `Class ~ ` 前缀）、`feature_match_key`（忽略大小写/空格/下划线/
  连字符/括号）、`ReplaceTarget`（`TYPE=<tag>` 锚点寻址 + 名字回退、
  容忍单复数漂移）、`line_owner`；
- `extensions/pf35e/data/pf35e_variants.lst` 的 `REPLACES` 改用 `TYPE=`
  ACF 锚点（`TYPE=FighterBonusFeat` / `TYPE=WizardFamiliar`）；Cleric 熟练行
  在 RSRD 里没有 ACF 标签，用规范化名字 `Cleric ~ Weapon and Armor Proficiency`；
- 修复后实测：Weapon Master / Armor Master 移除 `Fighter ~ Bonus Feats`、
  School Specialist 移除 `Wizard ~ Familiar`、Cloistered Cleric 移除
  `Cleric ~ Weapon and Armor Proficiency`，各自只移除该一条并保留其余；
- **修复 UI 分叉**：Web 侧 `matches_features` 原先用"子串包含"匹配**未清洗的
  原始字符串**，与引擎语义不同，导致 Cloistered Cleric / School Specialist
  被 `index.html` 直接隐藏（而引擎照加）。现改为复用同一套
  `ReplaceTarget` 匹配（`variant_matches_class_features`）；
- CLI 新增 `list-variants` 子命令：列出变体并标注每个 `REPLACES` 目标是否
  **真的命中**职业特性行（打不中显示 `UNRESOLVED(...)`），`--format json`
  提供 `all_targets_resolved`；
- 新增同名变体冲突诊断：两个变体替换同一锚点时在 snapshot `diagnostics`
  里报告"结果未定义"，而不是静默取后者；
- gate 新增第 4 层 semantics：断言每个 `REPLACES` 目标可解析（防止再出现
  "解析成功但打不中"的变体）。

### 其它

- `pcgen-web` 的规则包缓存改为 **路径 + 数据文件最新 mtime** 作键，并清理同
  路径的旧条目：开发中重新生成扩展 `.pcc` / 改 `.lst` 不再需要重启服务
  （此前缓存永不失效）；新增 2 个测试；
- `evaluate_class_formula` 的 4 条正则改用 `OnceLock` 缓存：该函数在每个职业
  每个等级、每个派生值上都会被调用，此前每次调用都重新编译正则；新增等价性
  与具体数值断言测试防止重构走样。

### PF 1e as 3.5e extension（变体层，ALPHA）

- `extensions/pf35e/` 改为**只提供变体层**：PF 的基础职业在 3.5e RSRD 里已存在且
  HD/BAB/豁免进度完全一致，重写没有意义；真正的差异是 archetype。
  新增 `data/pf35e_variants.lst`（`TYPE:Archetype` + `APPLIESTO` / `REPLACES` /
  `GRANTS` / `CSKILL`，4 个变体），删除 `data/pf35e_classes.lst`；
- `data/pf35e_abilities.lst` 改为变体引用的描述性能力条目，并修复此前每行缺少
  Tab 分隔（整行被当成一个名字）的问题；
- `scripts/pf35e_extension_gate.py` 重写为三层验收：layering（`game_mode == 35e`、
  职业数与 RSRD 相同、definition 增量可归因、叠加后无 pending tag）、
  parsing（每个变体进入 `CompiledRulePack.variants`）、behaviour
  （`create` → `level-up --archetype` 接受并算对 BAB；未知 archetype 与
  跨职业变体被拒绝；`evaluate` 产出 snapshot）；无数据时仍以退出码 2 显式 SKIP；
- **修复**：变体记录的 `APPLIESTO` / `REPLACES` / `GRANTS` 已由 `parse_variant`
  消费，但仍被计入 legacy pending，叠加 RSRD 后产生 17 个假 pending tag；
  现在 lowering 阶段显式豁免（新增回归测试 `crates/pcgen-ir/tests/variant_tags.rs`）；
- **修复**：变体记录同时进入 `abilities` 与 `variants`，导致同一个 archetype
  被算成一条能力；现在只进入 `variants`，`pack.abilities` 回到真实能力清单；
- **修复**：CLI `--out` 路径自 undo 功能（`CharacterRepository` 统一命名
  `<id>.json`）起被静默忽略，`create --out target/m9-smoke-character.json`
  实际写出 `target/smoke.json`，导致 `scripts/check.sh` 的 M9 smoke 失败；
  新增 `CharacterRepository::with_live_path` 固定 CLI 指定的路径（history 仍按
  角色 id 归档），并去掉重复打印的路径；
- **修复**：`create` / `level-up --out` 会把输出路径打印两次；
- 更新 `docs/PF35E_EXTENSION.md`（变体层设计、验收三层、覆盖度账目）与
  `PRODUCT_SCOPE.md` 的表述；

### M9-9：Web 建卡支持逐级多职业、格式塔与替代能力（进行中）

- `CreateRequest` 新增 `advancement` 字段，可显式描述每个角色等级的 class lane；
- `create_draft` 支持：
  - 普通多职业：每个等级选择不同职业；
  - Gestalt：每个等级同时选择两个职业 lane；
  - 每个 lane 可选 archetype / alternate ability 与 substitution level；
- 替代能力按职业已有特性索引：`variants` 增加 `matches_features`，UI 只展示与职业已有能力匹配的 Archetype / Alternate Ability；
- 性能优化：后端缓存职业特性集，前端预建 `ArchetypeByClass` / `SubstitutionByClassLevel` 索引，避免每次渲染全量遍历；
- 性能优化：Web 层增加 `CompiledRulePack` 内存缓存，`pack-options` / `recalculate` / `export` / character 打开/导入等请求不再重复编译全量规则包；
- 新增结构化搜索分页接口：`/api/options/spells`、`/api/options/equipment`、`/api/options/abilities`；
- 能力/专长搜索接口新增 `requires` 前置列表，前端候选显示“requires: ...”专长链提示；
- 支持 `SERVESAS` 职业变体：Cloistered Cleric 等服务 Cleric 的变体职业会作为 Cleric 的 Class Variant 显示；
- 依据 dndtools Lightbringer 页面补充 `Lightbringer Cleric`：Cleric 1 级替代等级 + 3 个替代能力；
- 字段与 UI 分离：class 等选项新增稳定 `uuid`，前端使用 UUID 作为字段值，显示名仍为 `name`，Web 请求时映射回内部 PCGen key；
- 修复 `.MOD` 显示：class 显示名自动剥离 `.MOD` 后缀；
- 修复职业变体混入主职业列表：`serves_as` 变体只出现在 Class Variant 下拉，不再出现在主职业下拉；
- 增加 `.pcc` 自动刷新：切换或修改 `pcc` 后自动重新拉取 pack-options / spells / equipment / abilities，无需手动点击刷新；
- 修复 `CLASS:X.MOD` 丢失当前职业上下文：Lightbringer Cleric 现在正确挂到 Cleric 1 级替代等级；
- 主职业列表不再混入 `serves_as` 变体和 prestige 进阶职业；灵能/进阶变体不再出现在基础职业下拉；
- 临时取消 `PREDEITY` 神祇前置：职业/能力不再因神祇要求被挡住，后续再做神祇适配；
- Level Editor 新增 `Prestige Class`：基础职业 + 职业变体 + 进阶职业分层选择；选择进阶后覆盖该 Lane 基础职业；
- 修复 Cleric 选变体后看不到 Cleric 替代等级的问题：变体/替代等级按基础职业查询；
- 技能点总量校验：builder 按职业等级 + 智力调整值计算可用技能点，超出报错；
- Review 显示技能点已用/可用，能力按类别分组显示；
- 种族也改为 UUID 字段 + 显示名分离，并通过 Web 层映射回内部 key；
- 种族支持“基础种族 + 种族变体”：`SERVESAS:RACE|...` 的亚种只出现在 Race Variant 下拉；
- 恢复 `renderSkillRows`，修复 Add Skill / Review 技能显示失效；
- class / race 选项新增 `type_tags`；
- 冲突处理：`SUBSTITUTIONLEVEL` 支持 `REPLACES`；前端检测 Archetype / Variant 与替代等级是否替换同一能力；
- 替代优先级：替代等级（特殊）优先于 Archetype（一般），UI 提示覆盖关系但不阻止创建；
- Review 显示能力类别计数与资源池统计，Add Skill/Ability 增加即时反馈；
- 新增 `POST /api/preview`：按当前创建请求返回服务端计算的 `ability_pool_usage`，Review 显示精确 spent / available / remaining；
- 新增 `GET /api/options/races` 结构化种族查询：类别/搜索/分页 + 种族详情；
- 种族选择器重构：搜索 + 类别过滤 + 基础种族 + 变体 + 信息预览；
- `RaceDefinition` 增加 `display_name`、`level_adjustment`；
- 新增 `GET /api/options/templates` 模板查询接口；
- Templates 选择器：搜索 + 添加/移除模板，Review 显示模板列表；
- `CreateRequest` 支持 `templates`，builder 校验并写入 `CharacterDraft.templates`；
- CLI `create` 新增可重复的 `--template` 参数，直接应用生物模板（按 id 或名称解析）；
- CLI `level-up` 新增 `--template`，可给已有角色追加生物模板（重复模板自动去重）；
- CLI 新增 `list-templates` 子命令：列出 `.pcc` 中的生物模板，支持 `--search` 过滤与 `--format json`；
- `list-templates` 新增 `--type-tag`（按类型标签过滤，可重复）与 `--with-details`（文本输出附带 CR/命骰/等级调整/标签）；
- Web 新增 `POST /api/characters/{id}/level-up`：给原生角色升级并可选追加模板，返回更新后的 snapshot 与 HTML sheet；
- Web 原生角色面板新增升级区：职业选择 + HP 输入 + 模板搜索选择器，升级后自动回写并刷新预览；
- `LevelUpRequest` 新增 `replace_templates` / `archetype` / `substitution_level` / `subclass`：升级可写入 lane 级选择并校验替代等级；
- CLI `level-up` 新增 `--replace-templates` / `--archetype` / `--substitution-level` / `--subclass`；
- Web 升级区新增目标等级、Archetype/替代等级/子职业选择与“替换全部模板”选项；
- `LevelUpRequest` 新增 `lanes[]`：同一角色等级可推进多个职业 lane（Gestalt）；
- builder 新增 `apply_level_up` / `preview_level_up`：核心升级逻辑与预演解耦，预览不改动原 draft；
- CLI `level-up` 新增 `--preview`（只预演）与 `--lane <class>` / `--lane-hp <n>`（多职业）；
- Web `POST /api/characters/{id}/level-up` 支持 `preview:true` 与 `request.lanes[]`；UI 增加 Preview 按钮与 Gestalt lane 行；
- 多 lane 升级现在写入同一个 advancement frame（Gestalt 语义），并支持 per-lane archetype/替代等级/子职业；
- 预览返回 `delta`（等级/HP/BAB/AC/豁免/技能点/属性/技能/新增能力差异），CLI `--preview` 同时在 stderr 打印摘要；
- CLI `level-up` 新增 `--lane-archetype` / `--lane-substitution` / `--lane-subclass`；
- Web 多 lane 行新增 per-lane 的 Archetype / 替代等级下拉与子职业输入；
- Web 升级预览并排渲染 HTML sheet（`levelUpSheet` iframe）；
- 角色存档在每次写入前按序号备份到 `.history/<id>/`，支持**多步**回退：新增 `CharacterRepository::undo` / `can_undo` / `history_depth`；
- `undo` 返回体与打开角色一致并带 `undo_depth` / `redo_depth`，Web 原生角色面板新增 Undo / Redo 按钮；
- 新增 `redo`（重做最近一次撤销）与 `GET /api/characters/{id}/history`（撤销/重做步数），CLI 增加 `redo` 子命令；
- 历史接口返回版本元数据（序号/等级/时间戳/所在栈），新增 `POST /api/characters/{id}/history/{n}/restore` 跳转到指定版本；
- Web 原生角色面板新增 History 面板（列出可恢复版本并一键 Restore）；
- Web 升级预览改为“当前 sheet / 预览 sheet”并排对比；
- CLI 新增 `history` 子命令（文本或 JSON 列出可恢复版本）；
- 历史版本附带相对当前版本的 `summary`（等级/职业/模板/能力**明细**），CLI 与 Web 历史面板均展示；
- Web 新增 `GET/POST /api/characters/bundle` 批量导出/导入，面板提供 Export/Import bundle 按钮；
- CLI 新增 `export-all` / `import-bundle` 批量打包命令；
- bundle 导入前校验 schema/应用版本（新增 `validate_bundle` / `BundleReport`），不兼容 schema 拒绝，版本差异给出警告；
- Review 技能点显示逐项来源（首级倍数 vs 普通级 + INT 调整）；
- Web 建卡向导：购点或技能点超支时阻止进入 Review 步骤并提示；
- 批量删除角色：Web `DELETE /api/characters/bundle`（多选 Delete selected）与 CLI `delete --id/--all [--dry-run]`；
- 建卡向导支持保存/加载草稿（浏览器 localStorage），并支持导出/导入草稿文件；
- `GET /api/characters?pcc=…` 按 campaign 规则指纹过滤角色列表；页面新增 “only current campaign” 开关控制是否过滤；
- 角色卡支持打印友好样式（`@media print` 仅保留 sheet 并全宽显示）；
- 建卡草稿支持 schema 迁移（`migrateCreateDraft`，旧格式自动升级）；
- `GET /api/characters` 新增搜索参数 `q` / `class` / `min_level` / `max_level`，页面提供搜索框；
- 搜索条件（pcc / 开关 / 搜索框 / 等级范围）持久化到 URL，刷新可恢复；
- 建卡向导支持自动保存草稿到 localStorage（防抖，状态栏显示 autosaved 时间）；
- 新增命名草稿库（Draft library）：可保存多份命名草稿、按名加载/删除；
- 草稿库支持整包导出/导入（JSON，导入时校验并跳过不兼容草稿，按名合并）；
- bundle 支持按规则指纹分组：`group_by_fingerprint` / `filter_by_fingerprint`，CLI `--list-groups` / `--pcc`，Web `GET /api/characters/bundle/groups`；
- Web 建卡向导：技能点预算实时校验条（spent / budget / remaining，超支高亮），INT、职业、等级变化即时刷新；
- Web 建卡向导：购点实时校验已有，`/api/preview` 的 `skill_points` 断言补充到测试；
- 新增 `POST /api/characters/{id}/undo` 与 CLI `undo` 子命令（`--dry-run` 只打印），可反复还原最近两个版本；
- `/api/preview` 返回 ECL、最终属性、技能点、专长槽、激活能力与诊断；
- Review 显示 ECL、最终属性、技能点、专长槽、模板效果与合法性诊断；
- Dragon Age 选择器扩展为 Life Stage：除龙外，其他生物若有幼体/成年体/古代体等年龄模板也会显示并纳入 Templates；
  - 法术支持按职业法表/等级/领域/学派/关键词筛选；
  - 装备支持按类型/槽位/基础物品/关键词筛选；
  - 能力/专长支持按类别/类型/关键词筛选；
- `pack-options` 不再下发全部法术/装备/专长，前端改为搜索式选择器；
- 后端新增替代等级与“当前职业等级”的强校验：等级不匹配直接报错；
- 修复 Archetype / Variant 与 Substitution Level 同时选择时的冲突：
  - 引擎现在允许同一 lane 同时应用 Archetype 与 Substitution Level；
  - 特征替换、职业技能、变体效果、施法者推进均按两者叠加处理；
- Web 建卡流程改为“步骤条 + 主从式编辑器”：
  - 步骤条：Basic → Advancement → Skills/Feats → Spells/Equipment → Review；
  - Advancement 使用左侧等级列表 + 右侧等级详情编辑器；
  - Normal / Gestalt 模式；
  - 支持 Add / Remove / Copy previous level，不使用拖拽；
  - 每个 lane 支持 Class + 替代等级/替代能力下拉；
  - 等级增删与职业汇总常驻；
- 技能与专长改为可视化选择器，不再依赖手写文本；
- `/api/pack-options` 新增 `abilities` 完整能力目录；
- 新增 `data/packs/custom/` 用于放置自定义资料包；
- 新增 3e/35e 全量扩展聚合包，并默认通过 `run-web.sh` / `run-web.bat` 优先载入；
- `/api/pack-options` 新增 `variants`，按职业列出可用的 archetype / substitution；
- `CharacterSummary.total_level` 统一改用 `CharacterDraft::total_level()`，
  兼容 `advancement` 角色；
- 新增 builder 与 web API 的逐级多职业/格式塔/替代能力回归测试；
- CLI `create` 适配新增 `advancement` 字段。

### M9-7：发布冻结（0.1.0，已完成）

- 新增 `RELEASE_NOTES.md`；
- `package_release.py` 输出 `release.json` 与 `SHA256SUMS`，zip 名称带版本；
- 实际生成 `dist/pcgen-rust-web/` 与 `pcgen-rust-web-0.1.0.zip`；
- 发布包包含 rsrd35 / srd30 数据包、run-web 启动器、打包与 beta 文档；
- 已验证发布二进制可加载两套数据包并返回 pack options。

### M9-6c：output sheet 深化（已完成）

- `render_html` 增加 Ability Scores / Combat / Saves / Skills /
  Feats & Abilities / Spellcasting / Equipment / Traits 分区；
- 打印友好 CSS（`@media print`、分页避免、diagnostics 隐藏）；
- HTML sheet 覆盖装备、法术位、prepared、domain spells、psionics；
- 新增 deep-section golden 断言。

### M9-6b：法术书编辑（已完成）

- builder 校验法术是否属于所选职业 list，并按 class level 自动推断法术等级；
- 修正 canonical helper 返回 map key 而不是 display id；
- Web pack options 的法术候选带 `class_levels`；
- Web 向导新增法术候选 datalist + Add spell 按钮，按职业过滤；
- 新增 Wizard + Magic Missile 建卡回归测试。

### M9-6a：装备购买 + 起始金币（已完成）

- builder 新增 3.5e 职业起始金币表 `starting_gold_for_class`；
- `CreateRequest.equipment_budget` 校验装备总花费，超出报错；
- CLI `create` 支持 `--equipment-budget` / `--starting-gold`；
- Web pack options 返回装备 `cost_gp` 与职业 `starting_gold`；
- Web 向导新增 Starting gold 输入并根据职业自动填充。

### M9-5a：Beta 自动化与反馈通道（已完成）

- 新增 `docs/M9_BETA_CHECKLIST.md`：朋友必测场景与反馈模板；
- 新增 `scripts/test_package.py`：数据包 / release 工具 dry-run smoke；
- `scripts/check.sh` 与 no-data CI 接入 package dry-run；
- Web 页面新增 Copy diagnostics 按钮，便于朋友反馈。

### M9-4：数据包打包 + 启动器（已完成）

- 新增 `scripts/package_datapack.py`：打包 `rsrd35` / `srd30` 玩家数据与
  `pack.json`；
- 新增 `scripts/package_release.py`：release binary + 数据包 + 启动器 + zip；
- 新增 `run-web.sh` / `run-web.bat`，自动定位数据包、角色目录并打开浏览器；
- 新增 `docs/M9_PACKAGING.md` 与 `data/packs/README.md`；
- 朋友 beta 验收转入 M9-5。

### M9-3：Web 创建向导 + 角色卡打印（已完成）

- 新增 `GET /api/pack-options`，返回 pack 内 race/class/skill/spell/equipment
  等候选；
- 新增 `POST /api/characters/create`，调用 `pcgen-builder::create_draft`
  并保存原生角色；
- `/api/characters/{id}` 增加 HTML sheet 输出；
- Web 页面新增建卡向导：购点实时花费、标准数组、技能/能力/法术/装备输入、
  创建后自动打开；
- 新增 HTML sheet 打印按钮；web smoke 10→12。

### M9-2：购点 + CLI create / level-up（已完成）

- `pcgen-builder` 新增 3.5e DMG 购点表、标准数组与预算校验；
- builder `create_draft` / `level_up_draft`：规则包校验、确定性 HP 平均、
  技能/能力/法术/装备规范化；
- CLI 新增 `create` / `level-up`，支持 `--out` 写原生 envelope；
- workspace 测试 213→220。

### M9-1b：Web 角色管理（已完成）

- `pcgen-web` 依赖 `pcgen-builder`，`WebConfig` 新增 `characters_root`
  （`--characters-root`，默认 `./characters`）。
- 新增 API：
  - `GET /api/characters`
  - `POST /api/characters`（从当前 `.pcc` + `.pcg` 保存原生角色）
  - `GET /api/characters/{id}`
  - `DELETE /api/characters/{id}`
- 页面新增原生角色列表、打开、删除、导入当前 `.pcg`。
- `NativeSaveEnvelope` 新增 `pcc` provenance；web smoke 9→10。

### M9-0 / M9-1a：个人建卡器范围与角色仓库（进行中）

- 冻结 M9 范围：购点、3.0+3.5e、打包数据、Web 优先、无随机 KIT。
- 新增 `pcgen-builder` crate：
  - `NativeSaveEnvelope`（schema + fingerprint + CharacterDraft）；
  - `CharacterRepository`（文件型多角色管理、原子写、schema 校验）。
- 新增 `docs/M9_PERSONAL_BUILDER.md` 与 `docs/M9_LOOP.md`。

### Sprint 3-L3b-4：prepared 语义与攻击总额（已完成）

- `.pcg` / `DraftSpell` 新增 `book`，只有 `BOOK:Prepared*` 才计入 prepared；
- Java prepared 只统计 memorized spells，并排除 racial innate；
- `CharacterSnapshot` 新增 melee / ranged / grapple 总额；
- deep-field 达到 7 case / 49 gated / 0 owned gaps。

### Sprint 3-L3b-3：Domain caster level（已完成）

- active domain 的 `DOMAINGRANTS` caster level 文本进入
  `CharacterSnapshot.caster_level_bonus`；
- 修正 `3e_CleElf` Law domain +1 caster level；
- deep-field 收敛到 24 gated fields / 4 owned gaps。

### Sprint 3-L3b-2：深度字段归一化修复（已完成）

- `equipment_ac_bonus` 合并 armor+shield；`skill_total` 只看 Java 训练技能；
  `spell_dc_max` 只比较共有 spell key；`caster_level_max` 仅双方都有时比较。
- deep-field 收敛到 7 case / **23 gated fields** / **5 owned gaps**；
  残留 prepared/caster/attack 转 L3b-3。

### Sprint 3-L3b-1：深度字段门禁（已完成）

- 新增 `deep_field_oracle.py` + deep field config/baseline，覆盖
  armor/shield bonus、skill_total、spell_dc_max、caster_level_max、
  prepared_count；攻击总额作为 visibility 冻结。
- 7 个代表 case、24 个 Java-gated 字段、14 条 owned known_gaps；
  默认拒绝新增 gap，stale gap 失败。
- Java 抽取器补充 melee / ranged / grapple 总额。
- full-data CI 与 `scripts/check.sh` 接入 deep-field gate。

### Sprint 3-L3a：COHORT_MATRIX 与未纳入裁决（已完成）

- 新增 `scripts/cohort_matrix.py` 与 `docs/COHORT_MATRIX.md`，42 PCG /
  32 XML / 18 HTM 全部归类。
- `35e_Q-PsiCrystal` 归为 companion，`none_derth` 归为 legacy；
  `pf_*` / `sf_*` 明确 out-of-scope。
- full-data CI 增加 cohort matrix `--check`。

### P7 设计 spike：SystemProfile + SaveEnvelope（已完成，未实现）

- 新增 `docs/P7_PROFILE_SAVE_ENVELOPE_SPIKE.md`，抽取 Tri20 的
  `SystemProfile` 与 `SaveEnvelope` 思路；不复制 Tri20 代码或 crate。
- 定义 `CompiledPackHeader.profile` / `GameSystemProfile` /
  `NativeSaveEnvelope` 的字段、迁移策略与分阶段验收。
- 明确 3.0/3.5e 第一阶段行为不变；MSRD / PF1 需产品确认后走独立 profile gate。

### Sprint 2 方案 A：primary / extended cohort 拆分（已完成）

- `broad_cohort.json` 只保留 3.0/3.5e 25 case；MSRD 2 case 移入
  `broad_cohort_extended.json` 与 `broad_baseline_extended.json`。
- `broad_oracle.py` 支持 `--cohort` / `--baseline`，主/扩展 cohort 各自
  冻结 baseline 与报告。
- `coverage_gate.py` 增加主 cohort scope 守卫，非 `3e_` / `35e_` case
  不能进入主门禁。
- full-data CI 增加 extended cohort path smoke 与 visibility report。
- 主 cohort 全部 Java 可比字段恢复 100%；MSRD 差异只在 extended
  visibility 中保留。

### Sprint 1 门禁可信度与可移植性（已完成）

- cohort 数据路径改为相对 data root；`broad_oracle` 支持
  `PCGEN_TESTDATA_DIR`，`java_root` 仅保留为 provenance。
- `--report` 拆分为 `all_field` / `gated` / `java_missing` 三张表，
  不再把 HTM 缺失字段计为分歧。
- `known_gaps` 改为 `{field, owner, issue}` 并进入契约门禁：与
  `java_gated_fields` 互斥、并集覆盖 Java 暴露字段、默认拒绝新增 gap。
- 新增 `--check-paths`；full-data CI 增加迁移数据目录 smoke。
- broad baseline / cohort schema 2→3。

### P6-1c Web 选择提示与导出下载（已完成）

- `GET /api/curated` 返回 `ability_candidates`（ability category 成员与可重复能力），
  页面提供候选 datalist。
- `POST /api/recalculate` / `POST /api/export` 返回 `unresolved_hints`，
  页面展示 unresolved choice 的 kind、候选与提示。
- 新增 Download `.pcg` 按钮；web smoke 测试 7→9，总测试 205→207。

### P6-1b HTM 变体与 XML cohort 扩展（已完成）

- HTM extractor 支持 `-cmb` / `-cmp`，含 touch / flat-footed AC。
- `35e_L7-3` 使用 `-cmb` sheet 并升级为全字段 Java-gated。
- 新增 `msrd_Elwood` / `msrd_Ilyana` XML case。
- 差异以 `known_gaps` 显式记录。
- broad cohort 25→27；新增 `scripts/test_htm_csheet.py` 并接入 full-data CI。

### P6-1a HTM csheet 解析与 cohort 扩展（已完成）

- 新增 `scripts/htm_csheet.py`，支持 `.htm` csheet 差分。
- broad oracle 支持 `java_sheet: "htm"`，不可比较字段自动跳过。
- cohort 23→25：`35e_L7-3(fighter-cleric-dwarf)` 与
  `35e_L10(fighter-human)`。
- 修复 `.pcg` 自定义装备 `BASEITEM` / `EQMOD` 解析与 `PLUSnA` /
  `PLUSnS` AC 折入；两个 HTM case AC 与 Java 一致。
- 测试 204→205。

### P5-14 KIT runtime 决策启动 + Pathfinder 立项评估（已完成）

- 新增 `docs/P5_KIT_RUNTIME_DESIGN.md`：冻结 KIT runtime 启动条件；
  实现维持 deferred。
- 新增 `docs/P5_PATHFINDER_ASSESSMENT.md`：Pathfinder 1e 评估完成，
  结论为 deferred / 不立项。

### P5-13 Web GUI 导出与提示（已完成）

- 新增 `POST /api/export`，返回可回写 `.pcg` 文本、
  `unresolved_choices` 与 `ability_pool_usage`。
- 页面新增 Export `.pcg` 按钮、导出文本面板、unresolved choice 提示、
  pool 剩余额度进度条。
- web smoke 测试 6→7；总测试 203→204。

### P5-12 Web GUI 增强（已完成）

- 新增 `GET /api/curated`，读取 broad cohort 作为 curated case 列表。
- `--cohort-root` 与 `--data-root` 分离，cohort 在仓库、数据在只读快照。
- `POST /api/recalculate` 支持 `overrides`：choices、abilities、
  active_states、class_subclasses、deity、domains。
- 页面新增 curated 下拉、choice 覆盖、ability pool 编辑、active state 输入。
- web smoke 测试 4→6；总测试 201→203。

### P5-11 本地 Web GUI（已完成）

- 新增 `pcgen-web` crate：axum 本地服务，`GET /`、`GET /api/health`、
  `POST /api/recalculate`。
- 页面展示 snapshot JSON / choices / unresolved_choices /
  ability_pool_usage / diagnostics / HTML 角色卡。
- Web API 复用 CLI 侧 `draft_from_pcg` 语义，保持 CLI 与 Web 一致。
- 新增 web smoke 测试（4 个）并接入 CI no-data 与 `scripts/check.sh`。
- 测试 197→201。

### P5-8 / P5-9 / P5-10 收尾（已完成）

- spell numeric v2：prepared buff 的 AC / save / stat / skill / attack
  `BONUS` 折叠进派生数值；已知但未准备的法术不会误算。
- broad `choice_state` 继续覆盖真实 PCG 的 choice / pool / domain 状态。
- KIT runtime / TEMPVALUE UI / Pathfinder 明确维持 deferred。
- 测试 193→197。

### P5-6 / P5-7 真实角色正确性基线与 spell numeric（已完成）

- broad `choice_state` 增加 `choice_bindings` / `ability_pool_usage` /
  `domain_spells` / `deity_domains` 基线。
- `CharacterSnapshot.applied_spell_effects` 输出 prepared / domain spell 的
  AC / save / stat / skill `BONUS`。
- 新增 `p5_runtime.rs` 断言真实角色选择、能力池、领域法术与法术数值效果。
- 测试 190→193。

### P5-5 PCG 选择元数据补强（已完成）

- `CHOSENTEMPLATE:[NAME:...]` 嵌套模板选择解析；chosen template 进入
  `templates` 与 `choices`。
- `DEITYDOMAINS:[DOMAIN:...]` catalog 与选定 `DOMAIN:` 行分离，分别进入
  `deity_domains` 与 `domains`。
- 信仰校验使用 `.pcg` domain catalog。
- `CharacterSnapshot.choice_bindings` / `deity_domains` 输出。
- 测试 185→190。

### P5-4 Choice Binding 与能力池计数（已完成）

- `EffectState.choice_bindings` 将 choice 绑定到发起 `CHOOSE` 的实体。
- `%CHOICE%` / `%LIST%` 与公式归一化优先使用 source-bound choice。
- `ability_pool_usage.selection_counts` 记录重复选择；`spent` 为真实计数。
- 重复选择但缺少 `MULT` 的能力写入 diagnostics。
- 测试 183→185。

### P5-3 Pathfinder 试运行与导入兼容修复（已完成）

- `.pcc` include 路径支持 `\` → `/` 归一化。
- 允许只有 `KEY:` 的 support `.pcc`。
- 数据文件条目支持 `|PRECAMPAIGN:...` 条件，路径只取 `|` 前部分。
- Pathfinder Core Rulebook 可完成 import-report 与基础 recalculate；
- Pathfinder 继续 deferred，不进入正式 coverage / acceptance gate。
- 测试 181→183。

### P5-2 PCG 选择元数据与能力池诊断（已完成）

- `CLASSABILITIESLEVEL` 的 `ADD:[...|CHOICE:...]` 进入 `PcgDraft.choices`。
- `SPECIALTIES:[SPECIALTY:...]` 进入 `PcgDraft.class_subclasses`。
- `.pcg DEITY` / `DOMAIN` 进入 `CharacterDraft.deity` / `domains`，并在
  class-level 非 gestalt draft 中生成 `DivineTrack`，进入 `faith_decisions`
  与 `domain_spells`。
- ability pool 超支写入 diagnostics，`ability_pool_usage.selected` 保留
  实际选中能力。
- 测试 178→181。

### P5-1 `.pcg` choice / subclass ingestion（已完成）

- `.pcg` `CLASS:...+SUBCLASS:...` 进入 `PcgDraft.class_subclasses`。
- `.pcg` `ABILITY:...+APPLIEDTO:...` 进入 `PcgDraft.choices`。
- CLI 转换把 choices / class_subclasses 传入 `CharacterDraft`。
- engine `apply_choose` 支持按 source entity 上下文解析 `draft.choices`。
- `PRESUBCLASS` 支持 `class_subclasses`。
- `TEMPLATESAPPLIED` 内嵌 `APPLIEDTO` 进入 choices；`CLASS|ALL` / `CLASS|PC` 动态 class list 解析。
- curated PCG 的 `unresolved_choices` 已归零；broad Java 字段未回退。
- 测试 174→178。

### P4 全量关闭（已完成）

- residual pending 清零：`ATTACKCYCLE` / `MOVECLONE` / `REACH` typed 消费并
  进入 `CharacterSnapshot`。
- language state：`LANGBONUS` / `AUTO:LANG` 进入 `languages`，`PRELANG`
  从实际语言集合求值。
- subclass state：`AdvancementLane.subclass` + `PRESUBCLASS` 真实求值。
- `ability_pool_usage` 输出 pool / spent / remaining / selected。
- bounded numeric tails：`ADD:SPELLCASTER|Arcane/Divine` PRC caster
  stacking；monster HD 无显式 HP 时使用基础 hit die 平均。
- 范围决策：`docs/P4_DECISIONS.md` 记录 KIT runtime deferred、
  Pathfinder deferred、G3 dropped、G4 superseded。
- ROADMAP 新增 P4 closure matrix；三数据集 pending 全部为 0，
  completion 100.0%；测试 168→174。

### 阶段 3：能力池 / 选择语义闭环（已完成）

- 新增 `AbilityDefinition` / `AbilityCategoryDefinition` typed 模型。
- `ABILITYLIST` 进入 `CharacterSnapshot.ability_lists`，`POOL` 变量进入
  `CharacterSnapshot.ability_pools`。
- `STACK` / `MULT` 进入 repeatable ability metadata，`CHOICE` 进入
  subclass class-choice metadata。
- `CHOOSE` 优先从 `draft.choices` 解析；动态但未提供选择的情况进入
  `unresolved_choices` 并写入 diagnostics，不再静默取 first/0。
- broad oracle baseline 新增 choice / unresolved / ability pool / list /
  repeatable ability 状态基线，覆盖现有 23-case cohort。
- 选择类 pending 清零；coverage baseline 重冻结为 DS-3E 1 / 100.0%，
  DS-35E 2 / 100.0%，DS-PSI 0 / 100.0%；测试 164→168。

### 阶段 2：剩余装备 / 法术 / 灵能 pending 清零（已完成）

- **P4-EQP-2**：`ALTCRITMULT` / `ALTCRITRANGE` / `ALTDAMAGE` /
  `ALTEQMOD` / `ALTTYPE` / `ARMORTYPE` / `BASEQTY` / `CHANGEPROF` /
  `WEAPONBONUS` typed 化；`EquipmentDefinition` 增加 alternate weapon
  profile，`WEAPONBONUS` / `AUTO:WEAPONPROF` 接入 `PREWEAPONPROF` 状态。
- **P4-SPELL-2**：`ADDDOMAINS` / `PROHIBITCOST` / `DONOTADD` / Spell
  `VARIANTS` 消费；条件 `ADDDOMAINS` 进入 `domain_spells`，
  `PROHIBITCOST` 进入 `CharacterSnapshot.prohibited_costs`。
- **P4-PSI-2**：Wilder 第三灵能职业 golden；`CharacterSnapshot.powers_known`
  输出；`ADD:SPELLCASTER|Psionic` 的 PRC 等级叠加到基础 manifester level。
- 三数据集目标 tag pending 为 0；baseline 重冻结为 DS-3E 35 / 98.9%，
  DS-35E 100 / 98.8%，DS-PSI 11 / 99.1%；测试 156→164。

### 阶段 1：typed-but-unconsumed 运行时闭环（已完成）

- **P4-CLASS-2**：`ClassProgression` 新增 `hit_die` / `skill_multiplier` /
  `levels_per_feat` / `add_levels` / `excluded_class` / `exchange_level`；
  `SKILLMULT` 进入首级技能点，`LEVELSPERFEAT` 进入 feat slot，`HITDIE`
  进入无显式 HP 输入时的平均 HP，`ADDLEVEL` 进入 effective class levels，
  `EXCLASS` / `EXCHANGELEVEL` 进入 snapshot `class_interactions`。
- **P4-STAT-2**：`DEFINESTAT:MINVALUE` 进入属性下限，`STATMOD` 进入
  派生属性修正，`SIZENUM` 进入 `size_rank` / size modifier。
- **P4-FAITH-2**：`VALIDFORDEITY` / `VALIDFORFOLLOWER` 进入 deity /
  alignment 信仰校验诊断。
- **P4-COMP-1**：CompanionMod 行按 `PRERACE` 选择，HD / SR / ABILITY /
  creature traits / `BONUS:COMBAT|AC` / `BONUS:STAT` 进入
  `CompanionSnapshot`。
- 新增 `class_progression.rs`、`stats_and_size.rs` 测试，并扩展
  `creature_traits.rs` / `faith_rules.rs`；测试 150→156。
- 真实数据 / broad oracle / archive differential 不变：DS-3E 75 /
  97.8%，DS-35E 192 / 97.7%，DS-PSI 35 / 97.2%；broad 23/23。

### P0.1 质量清偿 / 口径修正 / CompanionMod 数据保全（已完成）

- `CHOOSE` 不再同时计入 `lowered_effects` 与 `legacy_pending`；新增
  `pcgen-ir` lowering 回归测试，三个验收数据集的 `CHOOSE` pending 清零。
- `XPCOST` 按 P4-DEC-2 移入 out-of-scope allowlist，不再计入
  `semantic_pending`。
- CompanionMod `FOLLOWER:` 行改为按 id 分组保存：DS-35E 真实数据的
  108 行全部保留，不再被同 id 后续行覆盖；带 `PRERACE`/`PRE*` 的条件行
  先原样保存，runtime 暂不盲目叠加，避免 AnimalCompanion / Familiar
  加成重复计数。新增真实数据回归测试。
- coverage gate 收紧：DS-3E / DS-35E / DS-PSI 的 unsupported pending
  必须为 0；DS-35E unsupported allowlisted 精确允许 2 个 `PRETEXT`。
- 重新冻结 coverage baseline；`TAG_WORKLIST` 按 P0 后 DS-3E 重新生成，
  `TAG_TRIAGE` 标记为历史快照。
- 真实数据：DS-3E 75 / 97.8%，DS-35E 192 / 97.7%，DS-PSI 35 / 97.2%；
  unsupported pending 均为 0。测试 146→150。

### P0 工程收口（已完成）

- 仓库初始化 `git` 并提交当前 117 测试基线；`.gitignore` 排除
  `target/`、`.cargo-home/` 等生成物。
- 恢复 `../raw/pcgen-master/.pcgen-rust-testdata` marker；
  `coverage_gate.py` provenance 从 `unknown` 变为固定 lock ref。
- `ROADMAP.md` P0 关闭，阶段 0 口径冻结完成。

### P4-KIT-1 KIT / STARTPACK 声明式 typed 导入（已完成）

- `pcgen-core`：新增 `KitDefinition` / `KitEntry`，并加入
  `TypedDefinitions.kits` 与 `CompiledRulePack.kits`。
- `pcgen-ir`：解析 KIT/STARTPACK/TABLE 行，保留 `OPTION` / `LOOKUP` /
  `VALUES` / `QTY` / `EQUIPBUY` / `APPLY` / `LOCATION` 等表达式；
  `classify_for_category(category, tag)` 区分 KIT 与 Equipment 同名 tag，
  KIT tag 由 typed import 消费。
- 真实数据：DS-35E KIT pending 6,896→0，semantic completion
  45.5%→76.2%；DS-3E 67.6%→68.9%；测试 119→122。
- broad oracle / archive differential 行为不变（KIT 不执行随机展开）。

### 未训练技能展开 + 灵能/装备/职业/属性/信仰残留补齐（已完成）

- 未训练技能展开：带 typed bonus 的可使用未训练技能以 0 级基础
  （属性调整 + armor check）物化；trained-only 技能不物化。
- `MOVECLONE` 消费：race/class/template 的移动克隆展开进
  `CharacterSnapshot.movement`。
- CompanionMod runtime：新增 `CompanionModDefinition` typed 模型与
  `CharacterDraft.companions` / `CharacterSnapshot.companions`；
  `FOLLOWER:` 行的 `SR`（`mastervar(...)`）、`HD`、abilities 派生。
- P4-CLASS-1：`SKILLMULT` / `LEVELSPERFEAT` / `HITDIE` / `EXCLASS` /
  `ADDLEVEL` / `EXCHANGELEVEL` typed 到 `ClassDefinition`；race `SKILLMULT` /
  `XTRASKILLPTSPERLVL`。
- P4-STAT-1：`SIZENUM` / `STATMOD` / `DEFINESTAT` typed 到
  `SizeDefinition` / `StatDefinition` / race/template `stat_minimums`。
- P4-FAITH：`VALIDFORDEITY` / `VALIDFORFOLLOWER` typed 到
  `DeityDefinition` / `AlignmentDefinition`；`DEITYWEAP` 消费。
- 真实数据：DS-3E 170 / 95.1%，DS-35E 388 / 95.5%，DS-PSI 66 / 94.8%；
  broad oracle 23/23 100%。测试 146。

### P4-PSI-1 灵能闭环 + P4-BONUS/EQP 残留（已完成）

- `POWER`-style `SPELL:` 记录（`TYPE:Psionic`）不再计入普通 caster level；
  `SpellDefinition.is_psionic` 作为权威标记，`import-report` 新增
  `psionic_effects` 指标。
- engine 灵能闭环：`power_points` / `manifester_levels` / 灵能 DC /
  `spell_power_types` 输出；Psion、Psychic Warrior golden。
- `PRECHECKBASE` base-save 谓词 lower + 求值。
- `PREEQUIP*` primary / secondary / both 手部语义细分（基于武器 `WIELD`）。
- EquipMod `DR`/`SR` 在装备项通过 `.pcg` `EQMOD=` 引用时汇入装备 trait
  输出；`EquipmentModifierDefinition.traits` typed 化。
- 新增 DS-PSI 验收数据集（`_rsrd_psionics.pcc`）进入 coverage gate。
- 真实数据：DS-3E 318 / 91.1%，DS-35E 468 / 94.6%，DS-PSI 89 / 93.1%；
  broad oracle 23/23 100%。测试 142。

### P4-TEMP-1 active-state 模型（已完成）

- `Predicate::ActiveState` + `CharacterDraft.active_states`：临时状态显式激活，
  `TEMPBONUS` 不再作为静态 modifier 应用。
- `TEMPBONUS` 统一 lower 为带 `ActiveState` 条件的 `Effect`：
  `PC` / `ANYPC` 作用到角色，`EQ` 额外要求匹配装备类型。
- `TEMPVALUE` 解析为 `TemporaryValue { min, max, title }`，保存在
  `CompiledEntity.temporary_value`。
- `Spell` / `Skill` 实体可按 active state 激活；`CharacterSnapshot` 新增
  `active_states` 输出。
- `PREITEM` 谓词接入 `Predicate::Equipped`，清零 TEMPBONUS 因 `!PREITEM`
  条件产生的 unsupported。
- 真实数据：DS-3E `TEMPBONUS`/`TEMPVALUE` pending 清零，整体 pending
  331 / completion 90.8%；DS-35E 482 / 94.4%；broad oracle 23/23 100%。
  测试 134。

### 统一 typed bonus pipeline + 公式 stacking + creature trait owner 消费

- `EffectState` 将 `AcBonus` 泛化为 `TypedBonus { target, value, bonus_type,
  stack_key }`；新增 `STACK=` 解析；literal 与公式型 `BONUS` 都进入统一
  stacking 层。
- AC / saves / attack / skill 接入 typed stacking；
  `CharacterSnapshot` 新增 `attack_bonuses`。
- `CharacterSnapshot.spell_effects` 汇总领域法术与准备法术。
- `CompiledEntity.traits` 捕获所有 record 的 creature trait；
  `CompiledRulePack.creature_trait_records` 作为 metadata catalog；
  Race / Template / active ClassLevel / Ability / Feat / Equipment 会汇入
  snapshot 的 `vision` / `damage_reduction` / `spell_resistance` /
  `natural_attacks` / `reach_feet` / `reach_mult` / `unarmed_damage` /
  `unarmed_mult`。
- EquipMod / CompanionMod / `MOVECLONE` 未进入 runtime 消费，继续保持
  pending，等待 P4-EQP-1 / P4-RACE-1 处理。
- 真实数据：DS-3E pending 520 / completion 85.7%；DS-35E pending 858 /
  completion 90.2%；broad oracle 23/23 100%。测试 131。

### P4-BONUS/EQP 续作 + P4-SPELL-1 S2 + P4-RACE/TPL owner 归并（当前切片）

- 装备槽位：新增 `EquipSlot`、slot order 排序、slot conflict 诊断；
  `EquipmentDefinition` 增加 `slot` 推断；archive `equipment_slot_sort`
  意图已有 core 测试。
- 法术效果指标：`ImportReport.spell_effects` 统计 SPELL/DOMAIN 上的可执行
  effect；`import-report --json` 与 coverage gate 接入；DS-3E 16、DS-35E 41；
  新增 `spell_effects` golden 测试。
- Creature trait 输出：`CharacterSnapshot` 新增 `vision` /
  `damage_reduction` / `spell_resistance` / `natural_attacks`，从
  Race/Template `CreatureTraits` 聚合。
- 新增 P4-H owner 表，把 Ability/ClassLevel/Equipment/EquipMod/CompanionMod
  上剩余 creature-trait tag 归并到 P4-RACE-1 / P4-CLASS-1 / P4-EQP-1。
- 测试 123→127。

### P4-BONUS/EQP 第二阶段 + P4-SPELL-1 S1 + P4-RACE/TPL 分类（已完成当前切片）

- 残余 `BONUS` unsupported 清零；新增 `PREEQUIP*` / `PREWEAPONPROF` /
  `PREWIELD` / `PREDOMAIN` / `PREHD` / `PREVAR*` symbolic / `PREARMORTYPE` /
  `PREPROFWITHARMOR` / `PREPROFWITHSHIELD` / `PRESPELL` / `PRESPELLSCHOOL` /
  `PRESUBCLASS` / `PRELANG` predicate 解析与求值。
- 装备 typed 字段：`BASEITEM` / `CONTAINS` / `SLOTS` / `LOCATION` /
  `CHARGES`；DS-3E/DS-35E 这些 pending 清零。
- AC same-type non-stacking / dodge/circumstance/untyped stacking 切片；
  `TYPE=NaturalArmor.STACK` 保留 stack 语义。
- Spell/domain typed/消费：`DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` /
  `ADDSPELLLEVEL` / `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK`。
- Race/Template `CreatureTraits` typed：reach / natural attacks / DR / SR /
  CR / vision / subtypes / move clone 等；TemplateDefinition 加入
  `TypedDefinitions` 与 `CompiledRulePack`。
- 真实数据：DS-3E unsupported pending 0，completion 84.2%；
  DS-35E unsupported pending 0，completion 88.3%；broad oracle 23/23 100%。
- 测试 122→123。

### P4-SPELL-1 预研（已完成）

- 新增 [`docs/P4_SPELL1_PRERESEARCH.md`](docs/P4_SPELL1_PRERESEARCH.md)：
  按 category 统计 `DOMAINS` / `SPELLLEVEL` / `PROHIBITSPELL` /
  `ADDSPELLLEVEL` / `KNOWNSPELLSFROMSPECIALTY` / `SPELLBOOK` 与
  `PRESPELL` / `PRESPELLSCHOOL`，给出 S1 typed 定义 / S2 执行的分阶段验收。
- 预研不改变 engine 行为，可与 P4-BONUS/EQP 第二阶段并行。

### P4-RACE-1 / P4-TPL-1 / P4-DATA-1 第一阶段（已完成）

- `.pcg` 解析 `TEMPLATESAPPLIED`，Template entities 可激活。
- ability `.MOD` 合并到 base entity；`PRE*` 内嵌条件保留；`PRERACE` /
  `HasEntity` 支持 race/class。
- Halfling racial trait default 链修复，`3e_SWizSam` 三项豁免与 Java exact。
- Half-Dragon template 的 `BONUS:STAT`、natural AC 与 HP 生效，
  `35e_CloudGiantHalfDragon` / `35e_JimDop` 全字段与 Java exact。
- `CLASSABILITIESLEVEL:HITPOINTS` 按等级保留，HP 使用 per-level minimum；
  `3e_MonKee` HP 21→23。
- broad oracle：23/23 java-compared + 0 known-diverging，全部字段 100%。

### P4-ENG-1 类/等级实体激活与 modifier target 归一化（已完成）

- `pcgen-core`：`AttackProgression` 增加 `None` 并作为默认；没有显式
  `BONUS:COMBAT|BASEAB` 的职业不再获得默认半等级 BAB。
- `pcgen-engine`：class/class-level 实体激活与 progression lookup 统一
  stable key / display name（`charlie_class` ↔ `Charlie Class`）；
  `HP.CURRENTMAX`、`TL` 公式和 BASEAB/BASE.SAVE 去重归一化。
- 回归测试：`pcgen-engine/tests/effects.rs` 新增无 BASEAB 职业、带空格类 key、
  `HP.CURRENTMAX`、BASEAB/BASE.SAVE 不重复计数 3 个用例；测试 114→117。
- broad oracle：BAB 13/21→21/21（100%）；AC 17→20、touch 17→19、
  flat-footed 16→19、HP 15→17；`java_gated` 18→21；broad baseline 按
  Java 一致率收敛后重生成。

### P4-KIT-0 KIT / STARTPACK 设计 spike（已完成）

- 新增 [`docs/P4_KIT0_DESIGN.md`](docs/P4_KIT0_DESIGN.md)，明确 KIT/STARTPACK
  的两层范围：声明式 typed 导入优先（P4-KIT-1），运行时随机展开单独立项
  （P4-KIT-2，需产品确认）。
- 记录 `OPTION` / `LOOKUP` / `VALUES` / `EQUIPBUY` / `APPLY` / `QTY` 的
  DS-35E pending 数据、category 混用问题与 `classify(category, tag)` 前置条件。
- `ROADMAP.md` 将 KIT 任务拆为 P4-KIT-0（设计）与 P4-KIT-1（导入）。

### P4-BONUS-1 + P4-EQP-1 第一阶段（已完成）

- typed numeric AC 组件：armor/shield/natural 只进 AC/flat-footed，
  dodge 只进 AC/touch，deflection 等进三字段；修复 Charlie 盾/甲泄漏到 touch。
- `SaveProgression::None` 默认零值，修复无显式 `BONUS:SAVE|BASE.*` 的测试职业
  多算基础豁免。
- flat-footed 保留负 DEX 惩罚；zero-HD class 不再强制 1 HP。
- 公式解析支持 ability modifier（`WIS` 等）与单职业 `CL`，并把 `VAR.*`
  公式在 effect 公式前按依赖多轮求值；修复 3e_MonKee Monk AC 变量/条件。
- broad oracle：BAB/AC/touch/flat 21/21（100%），HP 20/21，save_* 20/21；
  cohort Java-gated 字段扩展；测试 117→119。

### P4-PRE-4 模板 / 手 / 移动谓词（已完成）

- `pcgen-core`：`Predicate` 增加 `TemplateCount` / `HandsAtLeast` /
  `HandsAtMost` / `MovementAtLeast`；`RaceDefinition` 增加 `hands` /
  `movement` / `templates`。
- `pcgen-ir`：`parse_race` 解析 `HANDS:`、`MOVE:Mode,ft`、`TEMPLATE:`；
  `parse_predicate` 支持 `PRETEMPLATE`（容错数据中的尾部 `]`）、`PREHANDSGTEQ`、
  `PREHANDSEQ`、`PREMOVE`。
- `pcgen-engine`：`EffectState` 注入手掌/移动/模板状态（race + draft 模板）；
  `CharacterDraft` 增加 `templates` 输入（`#[serde(default)]`）。
- `P4-PRE-6` 续：补 `PRESIZELT` / `PRESIZELTEQ` / `PRESIZEGT`。
- 真实数据：DS-3E `unsupported_tags` 16→3（四 tag 清零，顺带 lower 1 个 BONUS）；
  DS-35E 152→104。测试 110→114。

### P4-PRE-3 法术类型 / 法术书（已完成）

- `pcgen-core`：新增 `SpellPowerType { Arcane, Divine, Psionic, Unknown }`
  （与 `CastingType` 正交）；`ClassDefinition` 增加 `spell_power` /
  `requires_spellbook`；`Predicate` 增加 `SpellType` / `HasSpellbook` /
  `HasSpellPower` / `CastingTypeIs`。
- `pcgen-ir`：`parse_class` 读 `FACT:SpellType|...` 与 `SPELLBOOK:YES`；
  `parse_predicate` 支持 `PRESPELLTYPE` / `PRESPELLBOOK` / `PRESPELLCAST`
  （`MEMORIZE=` 与 `TYPE=` 两种形式）。
- `pcgen-engine`：新增阶段 A `derive_spellcasting_prereq()`（只读 class levels +
  静态类定义，不依赖 effects），`EffectState` 注入 spell state；
  `CharacterSnapshot` 增加 `spell_power_types` / `max_spell_level` /
  `has_spellbook`。
- 真实数据：DS-3E `unsupported_tags` 25→16（`PRESPELL*` 清零）；
  DS-35E 276→152（并顺带 lower 使用这些条件的 BONUS）。测试 105→110；
  coverage gate 11→12 pass（新增 DS-35E 阈值/zero-tag 门禁）。

### P4-PRE-6 谓词补齐（部分完成）

- `PRESIZEEQ` → `All(SizeAtLeast, SizeAtMost)`、`PRESIZEGTEQ` → `SizeAtLeast`；
  新增 `normalize_size()` 支持 PCGen 单字母尺寸（S/M/L/H/G/C…）。
- `PRETEXT` 改判 allowlist（纯展示文本）。
- 剩余 `PRETEMPLATE` / `PREMOVE` / `!PRESUBCLASS` / `PRELANG` /
  `PREWEAPONPROF` 需要新运行时状态，按 owner 并入 P4-TPL-1 / P4-EQP-1 /
  P4-RACE-1。

### P4-PRE-5：顶层 `!PRExxx` 取反（已完成）

- `pcgen-ir` 统一识别带 `!` 前缀的谓词 tag：
  - 新增 `pcgen_ir::tags::is_predicate_tag()`，同时匹配 `PRExxx` / `!PRExxx`；
  - `is_known_tag`、lowering 分支、BONUS 条件过滤、Grant 授权键过滤、
    class level 前置串同步改用该判定；
  - `parse_predicate` 对顶层 `!` 递归为 `Predicate::Not(..)`（原来只在
    `PREMULT` 的 `[...]` 组里处理 `!`）。
- DS-3E（回归代理，`srdbasic.pcc`）：`legacy_pending` 1,618→1,489，
  `semantic_completion` 65.4%→67.2%，`unsupported_tags` 保持 25；
  `!PRERACE` / `!PRETYPE` / `!PREABILITY` / `!PRECLASS` / `!PREAGESET` 清零。
- DS-35E（主验收，`_rsrd_basics.pcc`）：421 个 negated tag 离开 legacy，
  其中 399 个 lower 为 `Not(..)`；22 个正向形式尚未支持的
  （`!PRETEMPLATE` / `!PRESUBCLASS` / `!PREMOVE`）转入 unsupported。
- 修复：11 个带 `!PRE` 条件的 BONUS 由“静默去掉条件后生效”改为显式
  unsupported（DS-35E `lowered_effects` 7,768→7,757，属修复而非回归）。
  broad oracle `--check --json` `ok=true`，无 case 数值漂移。
- 测试数 99→104。

### 阶段 0：口径冻结与 coverage 非回归门禁

- 验收数据集固定：DS-3E `srdbasic.pcc`（回归代理）、
  DS-35E `_rsrd_basics.pcc`（主验收）；全量 RSRD 仅作信息报告。
- KIT / COMPANIONMOD / 怪物记录判定为 in scope，计入覆盖分母。
- 新增 `tests/fixtures/coverage/coverage_baseline.json`，冻结两个数据集的
  `entities` / `tag_occurrences` / `lowered_effects` / `semantic_pending` /
  `semantic_completion` 与逐 tag pending 次数。
- `coverage_gate.py` 原先的 histogram 检查只验证"各部分求和 == 总数"，
  数字增长也会通过；现改为**逐 tag 非回归**：任一 tag 的 pending 增加、
  lowered 下降或 completion 下降都会失败，同时允许 legacy ↔ unsupported
  的合法改判。
- 新增 `coverage_gate.py --write-coverage-baseline` 作为唯一的显式重置入口。
- `docs/TESTING.md` 新增验收数据集与基线说明；`PRODUCT_SCOPE.md` 记录
  KIT / COMPANIONMOD / 怪物 in scope；`ROADMAP.md` 新增 P0 阶段并清理旧状态头。
- 门禁 10→11 pass（0 fail / 0 skip）；`semantic_completion`
  DS-3E 67.2% / DS-35E 44.6%。
- `broad_oracle.py --write-baseline` 增加守卫：Java 字段一致率下降或
  `computed → data_missing` 时拒写，必须 `--accept-diff` 才能显式接受；
  正常写入路径已确认与旧 baseline 逐字节一致。
- backlog triage：新增 `docs/TAG_TRIAGE.md`；8 个展示/角色生成/UI tag
  （`AGEDIEROLL`/`BASEAGE`/`MAXAGE`/`HAIR`/`EYES`/`SKINTONE`/`SEX`/`ROLE`）
  改判 allowlist，DS-35E pending 9,367→9,242、completion 44.6%→44.9%；
  82 个 pending tag **全部有 owner**，新建 `P4-KIT-0`（DS-35E pending 的 ~72%）。
  测试 104→105。
- 决策落地：`P4-DEC-1` KIT / COMPANIONMOD in scope、`P4-DEC-2` G3 dropped、
  `P4-DEC-4` G4 superseded、`P4-DEC-3` Pathfinder 1e 正式延后（可复议）。
- 并行产出 `docs/P4_PRE3_SPELL_TYPE_DESIGN.md`（`SpellPowerType` +
  法术书 + 阶段 A/B/C 时序），P4-PRE-3 进入待实现状态。
- 阶段 0 未完成：backlog triage、data marker 恢复、`git init`。

### P4-PRE-1：PREVARNEQ / 残余 PREMULT

- `PREVARNEQ` lower 为 `Not(Variable { min, max })`。
- 新增 `PREPCLEVEL` → `Predicate::CharacterClassLevel { min, max }`；
  `ClassDefinition` 新增 `is_monster`，engine `EffectState` 新增
  `character_class_levels`，等级统计排除 Monster racial HD。
- 真实 3e SRD：`unsupported_tags` 38→35，`PREVARNEQ` / `PREMULT` 清零；
  `semantic_completion` 65.2%→65.3%。
- coverage gate 将 real 3e SRD 的 `unsupported_tags` 阈值 250→35，并新增
  `PREVARNEQ` / `PREMULT == 0` 断言。
- 测试数 92→95。

### P4-PRE-2：PRETOTALAB（已完成）

- 新增 `Predicate::TotalBab` 与 `PRETOTALAB:N` parser；lowering 产物为
  `TotalBab { min: N }`。
- 新增 `pcgen-engine/src/bab.rs`：
  - `compute_bab()` 统一 class progression / formula / gestalt / external
    `COMBAT.BASEAB`，最终 snapshot BAB 与 PRETOTALAB 共用同一路径；
  - `derive_prereq_bab()` 提供不依赖循环的 `BAB_prereq`，并接收
    `EffectState.total_bab`。
- `EffectState` 新增 `total_bab`；`apply_effects` 在 predicate 求值前派生；
  `Engine::evaluate` 改用 `compute_bab()`，`apply_external_modifiers` 不再
  重复添加 BAB。
- 真实 3e SRD：`unsupported_tags` 35→25，`PRETOTALAB` 清零；
  `semantic_completion` 65.3%→65.4%。
- coverage gate：real 3e SRD `unsupported_tags <= 25`，zero-tags 增加
  `PRETOTALAB`。
- 测试数 95→99。
- 时序设计见 `docs/P4_PRE2_PRETOTALAB_DESIGN.md`。

### 计划（Step 2）

- `ROADMAP.md` 的 P4 重写为可验收任务：每项补充目标、指标、门禁、
  完成定义、依赖/风险；按 PRE* 四组、装备/灵能/模板、SPELL effect、
  范围决策拆分。
- 新增 Java 字段一致率表作为排序依据；`ARCHIVE_TEST_MAP.md` 的
  G1–G4 已链接到 `P4-BONUS-1` / `P4-EQP-1` / `P4-DEC-2` / `P4-DEC-4`。

### 门禁效率与口径（Step 1）

- 新增 `scripts/pcgen_cli.py`：门禁脚本构建一次 CLI，并直接执行
  `target/debug/pcgen`，不再对每个 case 启动 `cargo run`。
- `scripts/broad_oracle.py --check --json` 输出结构化摘要，区分
  `computed` / `java_compared` / `known_diverging` / `java_gated` /
  `data_missing`；`--report --json` 同步输出字段一致率 JSON。
- 修复 broad oracle `--check` 中每个 case 被 `evaluate_case` 求值两次的问题。
- `scripts/coverage_gate.py` 改为读取 broad oracle 的 JSON 摘要做分母守卫，
  不再解析人类可读文本。
- `scripts/check.sh` 与 CI full-data 去掉重复的 broad oracle 调用：
  broad oracle 由 coverage gate 调用一次；CLI smoke 直接使用构建好的二进制。
- `docs/TESTING.md` 同步四层门禁、`testdata.lock.json` 升级流程与 JSON 口径说明。

### 架构

- 新增 `pcgen-ir` crate，承载运行时 IR（`RawRecord`、`PatchOperation`、
  typed definitions、`CompiledRulePack`、降低 pass、覆盖度）。
  依赖方向固定为 `pcgen-import → pcgen-ir ← pcgen-engine`。
- `pcgen-import` 只保留 PCGen 文件格式处理（`.pcc` 图、`.lst` tokenize、
  `.pcg` 读写），并 re-export IR 类型以保持调用点兼容。
- `pcgen-engine` 不再依赖 `pcgen-import`（仅 dev-dependency，供 loader+engine
  集成测试）。
- `PcgDraft` / `PcgClassLevel` / `PcgSpell` 数据移入 `pcgen-ir::draft`；
  `.pcg` 解析与写出仍在 `pcgen-import`。
- `PcgDraft → CharacterDraft` 转换从 engine 移到 CLI 适配层。
- 新增 `pcgen_core::EntityCategory` / `ChoiceKind`：
  - `CompiledEntity.category` 由 `String` 改为 `EntityCategory`（serde 仍
    序列化为原 PCGen 字符串，fingerprint 不变）；
  - engine 不再出现 `"RACE"` / `"CLASS_LEVEL"` / `"EQUIPMENT"` / `"PCSTAT"`
    等原始 category 字符串分支，改用 typed match。
- 新增 `scripts/check_architecture.sh` 护栏，并接入 `scripts/check.sh` 与 CI。

### 覆盖度口径

- 新增 `pcgen_core::tagclass`：所有“已识别但未 lower”的 tag 按三位一体分桶
  —— `Allowlisted`（展示/元数据，不计入分母）、`Pending`（legacy 待迁移）、
  `Unsupported`（lowering 拒绝，计入分母）。`TagBucket` 是报告用视图。
- `RuleCoverage` 新增 `legacy_allowlisted_by_tag` / `legacy_pending_by_tag` /
  `unsupported_pending_by_tag` / `unsupported_allowlisted_by_tag`，以及
  `legacy_allowlisted()` / `legacy_pending()` / `unsupported_pending()` /
  `semantic_pending()` / `semantic_completion()`。
- `semantic_completion` 分母 = `lowered + legacy_pending + unsupported_pending`。
  真实 3e SRD 完成率据此为 **46.8%**（此前的 49.5% 未计入 unsupported 337，属高估）。
- `import-report --histogram` 显示 pending legacy / pending unsupported /
  allowlisted 三组。
- coverage gate 断言两套划分都精确：
  `legacy_allowlisted + legacy_pending == legacy_tags`、
  `unsupported_allowlisted + unsupported_pending == unsupported_tags`。
- 文档数字漂移修正（当时值 77）：MIGRATION 标注为历史快照；当前值统一见 `ROADMAP.md`。

### 测试与门禁

- 测试数 72 → 79（当时快照；当前值见 `ROADMAP.md`）（新增 `tagclass`/`TagBucket` 单测与 `tag_split` 集成测试）。

### P3 清账

- broad oracle：cohort 每个 case 声明 `expectation`，新增 `data_missing_limit`
  与 `known_data_gaps`；报告分母固定为 `expectation_summary.computed`，
  data-missing 增长或 expectation 不符会失败。
- 新增 `docs/ARCHIVE_TEST_MAP.md`：archive 114 测试 → 新测试的
  migrated / superseded / dropped 对照表。
- `scripts/snapshot_diff_archive.py`：默认二进制路径指向仓库内 archive，
  升级为 **Java 裁决的三方差分**（archive vs new vs Java csheet），
  接入 `check.sh` 与 CI full-data。new 与 Java 一致而 archive 不一致时为
  `ARCHIVE-BUG`（不算失败）；archive 与 Java 一致而 new 不一致时为
  `REGRESSION`（失败）。
- 三方差分判定 8 个 case：**0 回归 / 0 divergence**；旧 archive 有 2 处自身 bug
  （`3e_BarJack` flat-footed AC、`3e_WizShar` literacy），新引擎在两处均与 Java 一致。
- `ARCHIVE_KNOWN_WRONG` 声明表会与实际观测比对，未再出现的条目打印 `STALE`。

## BONUS 家族 lowering（P4 启动）

- 新增谓词 `Predicate::SizeAtLeast` / `SizeAtMost` / `LegsAtLeast` /
  `RuleEnabled`；`pcgen-core` 导出规范尺寸序 `size_rank/size_at_least/size_at_most`。
- `parse_predicate` 支持 `PREBASESIZELT/LTEQ/EQ`、`PRELEGSGTEQ`、`PRERULE`、
  `PREVARGT/PREVARLT`、`PREAGESET`。
- engine `EffectState` 从种族解析 base size，`PRERULE` 默认开启（可用
  `choices["RULE:<name>"]` 关闭）。
- 真实 3e SRD：`lowered_effects` 2,944 → 3,103；`unsupported` 337 → 177
  （`BONUS` 167 → 8）；`semantic_completion` 46.8% → 49.4%。
- coverage gate 阈值收紧：`lowered_effects >= 3000`、`unsupported_tags <= 250`。
- 行为不变：curated Java 9/9、broad oracle 23 case、三方差分 0 回归。

## PRE 家族 lowering（P4 续）

- 新增谓词 `Predicate::AtLeast` / `RaceType` / `AgeCategory`。
- `parse_premult`：括号感知解析 `PREMULT:N,[..],[..]`，支持嵌套与 `!` 取反；
  `N == 组数` 时降为 `All`。
- `PRERACETYPE` → `RaceType`（engine 从种族 `type_tags` 求值）；
  `PREAGESET` → `AgeCategory`（未知年龄不匹配，保守，取代此前的 `Always`）。
- 真实 3e SRD：`lowered_effects` 3,103 → 3,107；`unsupported` 177 → 38
  （`PREMULT` 113 → 1）；`semantic_completion` 49.4% → 50.5%。
- 剩余 38 个 unsupported 全部是**建模缺口**（需要总 BAB / 法术类型 / 模板集合 /
  手数等运行时状态），已在 `docs/TAG_WORKLIST.md` 列表化。
- 行为不变：curated Java 9/9、broad oracle 23 case、三方差分 0 回归。

## Ability pool（P4 续）

- `CompiledEntity` 新增 `ability_category: Option<String>`；`ABILITY` /
  `ABILITYCATEGORY` 记录的 `CATEGORY:` 被捕获为 ability pool，不再计入 legacy。
- engine `find_entity` 解析 `Grant` 时按 pool 优先匹配（同 key 不同 pool 不会错配）。
- 真实 3e SRD：`legacy_tags` 4,289 → 3,348（`CATEGORY` 941 → 0）；
  `semantic_completion` 50.5% → 59.6%。
- 行为不变：curated Java 9/9、broad oracle、三方差分 0 回归。

## broad oracle 分母做实（P4 续）

- 修正 5 个 data-missing case 的 **PCC 映射**（此前映射反了）：
  `Test Nothing` / `Test46b` / `Test51H` / `Add SpellCaster` 在 PTS，
  `Giant` / `Monstrous humanoid` 在 RSRD。
- `JimDop` / `CloudGiant` 因缺 **monster HD progression** 无法算对，新增
  cohort expectation `computed_diverging`：必须能计算，但不要求与 Java 一致，
  且不计入一致率分母（数值仍冻结）。
- broad oracle：`18 → 23 compared`，**data-missing 0**；新增
  `must_compute` 与 coverage gate 的"分母不得缩小"守卫。
- 一致率（分母 21）：`final_stats/initiative 100%`、`save_fortitude 85.7%`、
  `armor_class 81.0%`、`bab 61.9%` 等，如实反映剩余差异。

## 装备元数据 + monster HD 修复（P4 续）

- **monster class BAB 公式修复**：`classlevel()`（空括号）未被正则匹配，
  导致 Giant / Monstrous humanoid 等怪物职业 BAB 全为 0。修正后
  `JimDop` BAB 2→6、`CloudGiant` 0→12，与 Java 完全一致。
- `class_progression_is_populated`：monster class 的空 progression 不再
  屏蔽其 formula BONUS。
- **装备元数据 typed 化**：`EquipmentModifierDefinition` 增加 `plus` /
  `item_types` / `replaces`；`EquipmentDefinition` 增加 `damage` /
  `crit_mult` / `crit_range` / `wield`。EQUIPMOD/EQUIPMENT 对应 tag 不再计入 legacy。
- 真实 3e SRD：`legacy_tags` 3,348 → 2,902；`semantic_completion` 59.6% → 65.2%。
- 行为不变（monster BAB 修复除外，属修正）：curated Java 9/9、broad oracle、三方差分 0 回归。

## 说明

更早的 Sprint 历史见 `MIGRATION.md`；当前状态见 `ROADMAP.md`。
