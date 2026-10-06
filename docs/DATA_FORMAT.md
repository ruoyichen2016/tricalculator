# 规范化数据格式设计（草案 v1）

> 状态：**设计草案**，尚未实现。
> 目的：把 PCGen 数据中**靠约定隐含**的信息，转成**结构上显式**的运行时格式。
> 相关：`docs/PF35E_EXTENSION.md`（变体机制）、`CHANGELOG.md`（本设计所依据的缺陷清单）。

---

## 0. 为什么要改：证据

不是"PCGen 的格式老"，而是**它的语义是隐式的**。本项目的缺陷几乎全部落在这条边界上。

### 0.1 缺陷分布（本仓库实测）

| 缺陷 | 所在层 | 隐式性来源 |
|---|---|---|
| `SUBCLASSLEVEL:` 劫持职业游标（两次） | `pcgen-ir` | **含义依赖顺序** |
| `SUBSTITUTIONCLASS:` 劫持职业游标 | `pcgen-ir` | 同上 |
| Wizard 全部职业特性未激活 | `pcgen-ir` | 同上 |
| `CATEGORY:ACF` 不被识别 | `pcgen-ir` | **同一概念多种写法** |
| `REPLACES` 匹配不上 | `pcgen-ir` | 同上 |
| `GRANTS` 逗号列表未拆分 | `pcgen-ir` | **值内嵌子语言** |
| `BONUS:VAR` 逗号列表未拆分 | `pcgen-engine` | 同上（已泄漏进引擎） |
| 多职业 `CL` 恒为 0 | `pcgen-engine` | 同上（已泄漏进引擎） |

### 0.2 架构守卫只挡住了语法，没挡住语义

`scripts/check_architecture.sh` 断言引擎不含 `.pcc` / `.lst`，这一条**通过**。但引擎里曾有：

```rust
target.strip_prefix("VAR.")              // PCGen 的 VAR. 语法
Regex::new(r"\bCL\b")                    // PCGen 的 CL / TL 记号
classlevel.replace_all(...)              // PCGen 的 classlevel() 公式
```

**两个最隐蔽的缺陷正是藏在这里**——守卫看不见语义泄漏。

> **已修复（独立于阶段 1）**：方言现在于转写期翻译成运行时词汇表
> （`pcgen_core::formula_vocab`），引擎只做变量查表，守卫新增语义检查。
> 详见 `ARCHITECTURE.md` §2.1。

### 0.3 当前模型只是"半规范化"

实测 RSRD 的 Wizard：

```text
DUMP class id=Wizard hd=d4 max=20 bab_prog=Half
     bab_formula=Some("classlevel(\"APPLIEDAS=NONEPIC\")/2")     ← 仍是 PCGen 公式字符串
     fort=Poor/Some("classlevel(\"APPLIEDAS=NONEPIC\")/3")
     will=Good/Some("classlevel(\"APPLIEDAS=NONEPIC\")/2+2")
DUMP prog features L1 count=6
     first=Some("Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar|!PREABILITY:1,CATEGORY=ACF,...")
                                                    ↑ 仍是未解析的 PCGen ABILITY 字符串
```

并且**同一件事有两份表示**：

| 表示 | 位置 | 形态 |
|---|---|---|
| 原始字符串 | `ClassProgression.features_by_level` | 未解析 |
| 已解析效果 | `ClassLevel` 实体的 `Grant` | 已结构化 |

这正是本项目反复出现的"同一判定写两份"模式，只不过这次是**数据**而非代码。

---

## 1. 设计原则

| # | 原则 | 消除的隐式性 |
|---|---|---|
| **R1** | **每条记录自述其种类** | 类型由"哪个 `.pcc` 指令引入"决定 |
| **R2** | **每条记录自述其归属** | 游标 / 顺序依赖 |
| **R3** | **不存在修改记录** | `.MOD`（35e 树 9,375 处）、`.CLEAR`（406） |
| **R4** | **一个概念只有一种写法** | `TYPE:Archetype` vs `CATEGORY:ACF` |
| **R5** | **公式是表达式树，不是字符串** | `classlevel("...")`、`CL`、`TL` |
| **R6** | **等级是字段，不是名字后缀** | `FighterArmorTraining1..4` |
| **R7** | **每条记录带溯源** | 出错无法定位到原始行 |
| **R8** | **格式有版本，加载即校验** | 静默产生错误数值 |

---

## 2. 格式总览

### 2.1 容器

一个资料包 = 一个目录：

```text
packs/rsrd-basics/
├── manifest.json          # 版本、来源、计数、校验和
├── classes.jsonl          # 每行一条记录
├── class_levels.jsonl
├── variants.jsonl
├── abilities.jsonl
├── races.jsonl
├── spells.jsonl
├── equipment.jsonl
└── ...
```

**为什么是 JSON Lines**

| 要求 | JSONL 满足 |
|---|---|
| 可 diff（缺陷变成数据 diff） | ✅ 一条记录一行，`git diff` 有意义 |
| 可流式加载（不必整体解析） | ✅ |
| 人类可读、无需专门工具 | ✅ |
| 无需 schema 工具链 | ✅ |
| 体积 | 23.7 MB（RSRD basics），可 gzip |

**为什么按种类分文件**：改一个职业只动 `classes.jsonl`，不产生无关 diff。

### 2.2 manifest

```json
{
  "format": 1,
  "pack_id": "rsrd-basics",
  "game_mode": "35e",
  "name": "3.5 RSRD Basics",
  "source": {
    "kind": "pcgen",
    "revision": "f91bd03f2dbe1145d5526b52aa3e48fcd0f2fc8d",
    "tag": "v6.09.08.RC1-NIGHTLY.20260911",
    "entry": "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    "transpiler": "pcgen-transpile 0.1.0"
  },
  "counts": { "classes": 64, "class_levels": 554, "abilities": 3228 },
  "checksums": { "classes.jsonl": "sha256:..." }
}
```

`source` 让"这份数据从哪来、用什么转的"可追溯；`checksums` 让"文件被改过"可检测。

---

## 3. 记录模式

每条记录是一个 JSON 对象，**必有** `kind`（R1）：

```json
{"kind": "class", "id": "Wizard", ...}
{"kind": "class_level", "class": "Wizard", "level": 1, ...}
{"kind": "variant", "id": "school-specialist", ...}
```

### 3.1 归属是显式字段（R2）

```json
// 旧：等级行是裸的 "1"，归属取决于前面出现过哪个 CLASS
1	ABILITY:Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar

// 新：归属写在记录里
{"kind": "class_level", "class": "Wizard", "level": 1,
 "grants": [{"feature": "Wizard ~ Familiar", "mode": "automatic"}]}
```

**游标劫持在这个格式里不可能发生**——没有游标可劫持。

### 3.2 溯源（R7）

每条记录可带：

```json
"origin": {"file": "basics/rsrd_classes.lst", "line": 387}
```

一个数值错了，直接定位到原始行。这对本项目的调试方式（对照官方文本逐条核对）是必需的。

---

## 4. 各域记录

### 4.1 `class`

```json
{
  "kind": "class",
  "id": "Wizard",
  "name": "Wizard",
  "origin": {"file": "basics/rsrd_classes.lst", "line": 362},
  "hit_die": 4,
  "max_level": 20,
  "is_prestige": false,
  "base_attack": {"kind": "half"},
  "saves": {"fortitude": "poor", "reflex": "poor", "will": "good"},
  "skill_points_per_level": 2,
  "class_skills": ["Concentration", "Craft", "Decipher Script", "Knowledge", "Profession", "Spellcraft"],
  "spellcasting": {
    "ability": "INT",
    "power": "arcane",
    "requires_spellbook": true
  },
  "spell_slots": {"1": [3, 1], "2": [4, 2], "...": []}
}
```

要点：

- **`base_attack` 是枚举/结构，不是公式字符串**（R5）。`classlevel("APPLIEDAS=NONEPIC")/2` 这种写法在转写期就被归约成 `{"kind":"half"}`；若确实无法归约，则用 §5 的表达式树。
- `spell_slots` 的键是**等级**，不是名字的一部分（R6）。

### 4.2 `class_level`

```json
{
  "kind": "class_level",
  "class": "Wizard",
  "level": 1,
  "origin": {"file": "basics/rsrd_classes.lst", "line": 384},
  "grants": [
    {"feature": "Wizard ~ Weapon and Armor Proficiency", "mode": "automatic",
     "unless": {"kind": "replaced", "anchor": "WizardProficiencies"}},
    {"feature": "Wizard ~ Spells", "mode": "automatic"},
    {"feature": "Wizard ~ Familiar", "mode": "automatic",
     "unless": {"kind": "replaced", "anchor": "WizardFamiliar"}}
  ]
}
```

要点：

- **`class` 字段显式**（R2）。
- `unless` 把原来内嵌在 `ABILITY:` 值里的 `!PREABILITY:...TYPE.X` 谓词**提升为一等字段**。
- **这是 `features_by_level` 与 `ClassLevel` 实体的唯一表示**——消除 §0.3 的双份表示。

### 4.3 `variant`

```json
{
  "kind": "variant",
  "id": "school-specialist",
  "name": "School Specialist",
  "applies_to": "Wizard",
  "origin": {"file": "extensions/pf35e/data/pf35e_variants.lst", "line": 41},
  "replaces": [{"anchor": "WizardFamiliar"}],
  "grants": [
    {"feature": "Arcane School", "at_level": 1},
    {"feature": "Arcane Bond", "at_level": 1}
  ]
}
```

要点：

- **一个概念只有一种写法**（R4）：不再区分 `CATEGORY:Archetype` 与 `CATEGORY:ACF`，输出统一为 `variant`；输入方言的差异在转写期消化。
- `replaces` 是**结构化锚点**，不是靠名字模糊匹配。
- `at_level` 是**字段**（R6），对应原来 `GRANTS:X|PRECLASS:1,Wizard=5`。

### 4.4 `ability` / `feat` / `spell` / `equipment` / `race`

同构，各自字段显式。示例（`ability`）：

```json
{
  "kind": "ability",
  "id": "Wizard ~ Familiar",
  "name": "Familiar",
  "category": "Special Ability",
  "type_tags": ["ClassFeatures", "WizardClassFeatures", "SpecialQuality"],
  "origin": {"file": "basics/rsrd_abilities_class.lst", "line": 506},
  "effects": [{"kind": "set_variable", "name": "FamiliarLVL", "value": {"kind": "class_level", "class": "Wizard"}}],
  "description": "A wizard can obtain a familiar ..."
}
```

注意 `value` 是**结构化引用** `{"kind":"class_level","class":"Wizard"}`，而不是 `WizardLVL` 或 `CL` 这种记号（R5）。

---

## 5. 公式表示（R5）

复用已有的 `pcgen-formula::Expr`，但**把 PCGen 专有记号在转写期消解**：

| PCGen 写法 | 规范化后 |
|---|---|
| `CL` | `{"kind":"class_level","class":"<所属职业>"}` |
| `TL` | `{"kind":"character_level"}` |
| `classlevel("APPLIEDAS=NONEPIC")` | `{"kind":"class_level","class":"<所属职业>"}` |
| `classlevel("Wizard")` | `{"kind":"class_level","class":"Wizard"}` |
| `var("Foo")` | `{"kind":"variable","name":"Foo"}` |
| `3+CHA` | `{"kind":"binary","op":"add","left":3,"right":{"kind":"stat_modifier","stat":"CHA"}}` |

**关键**：`CL` 的所属职业必须在**转写期**确定（从记录来源），而不是留到运行时靠 `source_key` 猜——那正是多职业下 `CL` 恒为 0 的根因。

无法归约的表达式**保留 AST 而非字符串**，引擎只做算术，不再解析文本。

---

## 6. 加载与校验（R8）

加载器必须校验并**拒绝**：

| 校验 | 防止 |
|---|---|
| `format` 版本已知 | 旧格式被新引擎静默误读 |
| `kind` 在允许集合内 | 未知记录被忽略 |
| `class_level.class` 指向已声明职业 | **游标劫持的等价物** |
| 每个 `class` 至少有一个 `class_level` | 特性静默丢失 |
| `variant.replaces` 锚点可解析 | 替换静默不生效 |
| `checksums` 匹配 | 文件被改动 |

这些正是 `tree_scan` 现有不变式的**结构化版本**——在旧格式里它们需要专门扫描才能发现，在新格式里**加载即拒绝**。

---

## 7. 前后对照（真实数据）

### 输入（PCGen，`rsrd_classes.lst`）

```text
362	CLASS:Wizard	HD:4		TYPE:Base.PC	MAXLEVEL:20	BONUS:COMBAT|BASEAB|classlevel("APPLIEDAS=NONEPIC")/2|TYPE=Base.REPLACE
367	SUBCLASS:Abjurer		COST:2	PROHIBITCOST:1	CHOICE:SCHOOL|Abjuration
368	SUBCLASSLEVEL:1	ABILITY:Special Ability|AUTOMATIC|Abjurer Learning Bonus
384	1	ABILITY:Wizard Class Feature|AUTOMATIC|Wizard ~ Weapon and Armor Proficiency|!PREABILITY:1,CATEGORY=ACF,TYPE.WizardProficiencies
387	1	ABILITY:Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar|!PREABILITY:1,CATEGORY=ACF,TYPE.WizardFamiliar,TYPE.WizardSummonFamiliar
```

**隐式信息**：第 384/387 行的归属，取决于"前面出现过 `CLASS:Wizard` 且中间的 `SUBCLASSLEVEL:` **没有**被误认为职业头"。这条推断曾经出错两次。

### 输出（规范化）

```json
{"kind":"class","id":"Wizard","hit_die":4,"max_level":20,"base_attack":{"kind":"half"},
 "saves":{"fortitude":"poor","reflex":"poor","will":"good"},"origin":{"file":"basics/rsrd_classes.lst","line":362}}
{"kind":"class_level","class":"Wizard","level":1,
 "grants":[{"feature":"Wizard ~ Weapon and Armor Proficiency","mode":"automatic","unless":{"kind":"replaced","anchor":"WizardProficiencies"}},
           {"feature":"Wizard ~ Familiar","mode":"automatic","unless":{"kind":"replaced","anchor":"WizardFamiliar"}}],
 "origin":{"file":"basics/rsrd_classes.lst","line":384}}
{"kind":"variant","id":"abjurer","applies_to":"Wizard","origin":{"file":"basics/rsrd_classes.lst","line":367}}
```

**隐式信息全部消失**：归属是 `"class":"Wizard"` 字段；替换条件是 `unless` 字段；等级是 `"level":1` 字段。

---

## 8. 这个设计**不做**什么

| 不做 | 原因 |
|---|---|
| 不手工重写 40,000 行 | 用转写器（transpiler）从现有数据生成 |
| 不放弃原始数据 | 保留 `raw/` 作为来源，转写器可重跑 |
| 不改游戏规则语义 | 数据内容不变，只换表示；差分 oracle 仍有效 |
| 不引入二进制格式（初期） | 可 diff 优先于体积；需要时再加 gzip |
| 不在运行时保留 PCGen 记号 | `VAR.` / `CL` / `classlevel()` 只存在于转写期 |

---

## 9. 分阶段落地

| 阶段 | 内容 | 验收 |
|---|---|---|
| **1** | 职业 + 变体（bug 最密集处）：定义格式、写转写器、加载器、校验 | `tree_scan` 不变式全绿 + 差分测试全绿 + PF35e gate 全绿 |
| **2** | 扩展域：ability / feat / spell / equipment / race | 同上 + 覆盖率不低于现状（76.6%） |
| **3** | 退役运行时 PCGen 路径（保留转写器与原始数据） | 架构守卫加严：禁止 `VAR.` / `CL` / `classlevel` 出现在引擎 |

### 阶段 1 要回答的关键问题

1. 规范化后的数据能否通过**全部现有验收**？
2. 转写器的输出 diff 是否**可审查**（缺陷是否真的变成可见的数据 diff）？
3. 加载期校验能否**替代** `tree_scan` 的扫描（即"加载即拒绝"）？

若三个都"是"，阶段 2 是重复劳动；若否，代价可控。

---

## 10. 阶段 1 实施结果（已完成）

实现位于 `crates/pcgen-transpile`：`format`（记录类型）、`transpile`（转写）、
`pack`（读写 + 校验）、`reconstruct`（还原为运行时定义）。

### 10.1 三个关键问题的答案

**问题 1：规范化后的数据能否通过全部现有验收？——能。**

等价性测试 `tests/equivalence.rs` 两条：

| 测试 | 内容 | 结果 |
|---|---|---|
| `class_domain_round_trips` | 职业域逐字段往返一致 | ✅ |
| `engine_output_is_unchanged` | 10 组职业×等级的引擎输出（BAB/豁免/HP/法术位/激活能力/驱散/反射闪避/专长槽）完全一致 | ✅ |

全量门禁：`tests=365 failed=0`、`coverage gate pass=15 fail=0`、
`three-way differential 0 regression / 0 divergence`、PF35e gate 通过。

**问题 2：输出 diff 是否可审查？——是。** JSONL 一记录一行，按类型分文件。

**问题 3：加载期校验能否替代扫描？——能，而且立刻见效（见 10.2）。**

### 10.2 加载期校验立刻发现两个真实缺陷

这是本次实施最有价值的产出。校验一上线就拒绝了 RSRD 包，追查后是两个
**既有的、门禁没发现的**导入缺陷：

| 缺陷 | 现象 | 影响 |
|---|---|---|
| **`N:REPEATLEVEL:M` 被当作职业头** | `14:REPEATLEVEL:4` 变成名为 `14:REPEATLEVEL:4` 的"职业" | RSRD 2 个、另有 3e 若干；这些等级行自己的效果脱离任何职业 |
| **`.lst` 文件头被当作职业头** | `SOURCELONG:Revised (v.3.5) ...` 变成"职业" | RSRD 1 个、DS-3E 1 个 |

修复后：**RSRD 职业数 64 → 61**、**DS-3E 43 → 42**，实体数与 lowered_effects **不变**
（15761 / 8196）——即没有丢数据，只是不再凭空造职业。

两个修复都用**共用谓词**（`class_level_of_name` / `is_file_metadata_row`），
两遍 lowering 同时受益，避免再次"只修一份"。

DS-3E 的 `classes` / `progressions` 下限相应从 43 降到 42，理由写在
`coverage_gate.py` 内联注释里（沿用此前 45→43 的写法）。

### 10.3 一条被证伪的校验规则

我最初把"职业必须至少有一个等级行"写成硬校验。实测发现它**有两类合法例外**：

- **怪物（种族 HD）职业**：RSRD 有 27 个，进度来自 hit dice 而非 `CLASS:` 等级行；
- **隐藏的法术列表载体职业**：如 `Sorcerer/Cleric (Arcane)`（`VISIBLE:NO`），
  只用来挂法术列表。

而 `ClassDefinition` **没有可见性字段**，无法区分。因此该规则**降级为诊断警告**
（`LoadedPack.warnings`），而不是伪装成一条会误报的硬规则。阶段 2 补上可见性字段后
可以再提升为错误。

### 10.4 转写期发现并修正的两处保真问题

等价性测试暴露了两个转写器自身的缺陷，都已修复：

- **`levels_per_feat` 等字段丢失**：`ClassRecord` 最初只挑选了部分字段，
  丢掉 `levels_per_feat` 直接导致战士 1 级的专长槽从 1 变 2。
  改为**整体携带 `ClassProgression`**；
- **非替换类守卫被丢弃**：`PRECLASS:1,Ex Paladin=3` 这类守卫不是替换锚点，
  最初被丢掉。现在 `unless_replaced`（结构化锚点）与 `guards`（其余守卫，逐字保留，
  阶段 2 结构化）分开携带。

另外 `unless_category` 单独携带：术士的魔宠守卫在 `Special Ability` 池，
法师的在 `ACF` 池，硬编码任一都会错。

### 10.5 阶段 1 的边界（诚实声明）

- 只有**职业域**规范化：`class` / `class_level` / `variant`。
  `ClassDefinition` 的其余字段（`level_bonuses`、`prohibited_spells`、
  `spell_levels` …）**原样携带，仍含 PCGen 约定**，阶段 2 展开；
- `guards` 中的非替换守卫仍是**字符串**，未结构化为表达式（R5 部分满足：
  进阶公式已消除，谓词未）；
- 转写器**复用** `pcgen-ir` 的现有翻译，因此阶段 1 并未删除那 3,252 行——
  只是把它的运行时机从"每次加载"变成"构建期一次"。删除是阶段 3。
