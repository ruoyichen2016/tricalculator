# PF 1e 作为 3.5e 扩展（变体层）— 设计与范围

> 状态：**已落地最小可用切片（ALPHA）**。
> 实现：`extensions/pf35e/`、`scripts/package_pf35e_extension.py`、
> `scripts/pf35e_extension_gate.py`。
> 相关：`docs/P5_PATHFINDER_ASSESSMENT.md`（原 deferred 结论）、
> `docs/P5_PATHFINDER_TRIAL.md`（试运行记录）、
> `docs/P7_PROFILE_SAVE_ENVELOPE_SPIKE.md`（profile 抽象）。

## 1. 定位

Pathfinder 1e 在本项目里**不作为独立 game system，而是作为一个
`GAMEMODE:35e` 的扩展包**——与 3e/3.5e 生态里大量第三方扩展
（Unearthed Arcana、Complete 系列、Paradigm Concepts 等）同一种形态：

- 扩展自己声明 `GAMEMODE:35e`；
- 通过 `PCC:@/<rsrd 路径>` **挂在 RSRD 之上**；
- 只用 3.5e 语法表达；
- 建档、求值、出口全部走既有 3.5e 引擎，**不新增 profile、不改 engine 分支**。

与"官方 PCGen Pathfinder 数据集"的区别：

| | 官方 PF 数据集 | 本扩展 |
|---|---|---|
| GAMEMODE | `Pathfinder`（独立） | `35e` |
| CMB/CMD | 原生 | 不可表达（见 §4） |
| 数据来源 | Paizo 数据原样 | 本项目**重写** |
| `.pcg` 兼容 | PF `.pcg` 直接读 | 需按 3.5e `.pcg` 另存 |

## 2. 为什么不能直接拿官方 PF 数据当扩展

实测结论（见 `P5_PATHFINDER_TRIAL.md` 与本次复核）：

- 官方 PF 数据声明 `ALLOWEDMODES:Pathfinder|Pathfinder_RPG`，与 `35e` 互斥；
- PF 数据使用 3.5e 不存在的语义：`charbonusto(...)`、`Caster_Level_*`、
  `ACTYPE:CMD`、`skillinfo(...)`、favored class bonus、archetype 等；
- 直接挂到 3.5e pack 上会静默丢失/算错（实测 Java 对照 28 字段仅 13 对）。

因此本方案**不消费官方 PF 数据集**，而是把 PF 内容用 3.5e 语法**重新写一遍**。

## 3. 已落地切片（ALPHA）

### 3.1 只做变体层，不重写基础职业

PF 的基础职业（Cleric / Fighter / Wizard / Rogue / Barbarian / Bard /
Druid / Sorcerer …）在 3.5e RSRD 里**已经存在，且 HD / BAB 公式 / 豁免
进度完全一致**——PF 在这些项目上没有改规则，重写一遍只会制造重复定义和
维护负担。

PF 与 3.5e 的真实差异在**职业特性**：PF 用 archetype（变体）替换/追加
职业能力。因此本扩展只提供这一层：

```text
extensions/pf35e/
  rsrd_pf35e.pcc.in          # 模板（RSRD 路径由脚本注入）
  data/pf35e_variants.lst    # TYPE:Archetype + APPLIESTO/REPLACES/GRANTS/CSKILL
  data/pf35e_abilities.lst   # 变体引用的能力条目（描述性，无数值机制）
```

`scripts/package_pf35e_extension.py` 把扩展复制到
`<pcgen-data>/data/_pf35e_extension/`，生成可加载的 `rsrd_pf35e.pcc`。

### 3.2 当前覆盖

4 个变体，全部走引擎已有的 variant 机制。`REPLACES` 用 `TYPE=` 寻址
（见 §3.4），替换目标都经过"能真正命中"的验收：

| 变体 | 适用职业 | 替换（已核实命中） | 追加（@ 为获得该特性的职业等级） |
|---|---|---|---|
| Cloistered Cleric | Cleric | `Cleric ~ Weapon and Armor Proficiency` | Cloistered Cleric@1 |
| Weapon Master | Fighter | `TYPE=FighterBonusFeat` → `Fighter ~ Bonus Feats` | Weapon Training@5 / Weapon Guard@9 |
| Armor Master | Fighter | `TYPE=FighterBonusFeat` → `Fighter ~ Bonus Feats` | Armor Training@3 / Armor Mastery@19 |
| School Specialist | Wizard | `TYPE=WizardFamiliar` → `Wizard ~ Familiar` | Arcane School@1 / Arcane Bond@1 |

`data/pf35e_abilities.lst` 为上述变体引用的能力名提供描述性条目，
使角色卡能显示名称（无数值机制）。

Cleric 那一项用名字寻址而非 `TYPE=`：RSRD 的 Cleric 熟练行**没有** ACF
类型标签（Fighter/Wizard/Sorcerer 有），所以用规范化名字作为回退。

### 3.2.1 等级化 `GRANTS`

追加的特性可以声明获得等级，写法与 PCGen 在真实 archetype 的 `ABILITY:`
行上使用的 `PRECLASS` 守卫一致：

```text
GRANTS:Weapon Training|PRECLASS:1,Fighter=5
```

- 没有守卫 = 1 级即可用（等同 PCGen 对裸 `ABILITY` 标签的处理）；
- 引擎在 `features_for_class` 里按 lane 的当前职业等级过滤，
  所以 1 级 Weapon Master 不会提前列出 Weapon Training；
- `REPLACES` **不受等级影响**：被替换掉的特性全程消失。

```bash
target/debug/pcgen list-variants --pcc <pcc>
# Weapon Master  ...  grants=Weapon Training@5,Weapon Guard@9  ok
```

### 3.3 `REPLACES` 的寻址方式

`REPLACES:` 支持两种写法，实现在 `pcgen_core::ReplaceTarget`：

| 写法 | 含义 | 何时用 |
|---|---|---|
| `TYPE=FighterBonusFeat` | 匹配 PCGen 自己用来标记"此特性可被替换"的 ACF 类型标签（特性行里的 `!PREABILITY:1,...,TYPE.FighterBonusFeat`） | 数据里有该标签时**优先**用；它是稳定锚点，改显示名不影响 |
| `Cleric ~ Weapon and Armor Proficiency` | 匹配特性显示名 | 数据里没有 ACF 标签时（如 Cleric 熟练行） |

名字匹配会规范化：忽略大小写、空格、下划线、连字符、括号，剥离
`Class ~ ` 所有者前缀与 `!PRE...` 谓词，并容忍 PCGen 的单复数漂移
（`Fighter Bonus Feat` ↔ `Fighter ~ Bonus Feats`）。

> **为什么不是"结构化全名"也不是"别名映射表"**：结构化全名（`Fighter ~
> Bonus Feats`）把数据钉死在某个 RSRD 版本上，改一个显示名就静默失效；
> 别名映射表则要额外维护一张表，本身成为新的失效点。`TYPE=` 锚点是数据
> 里**本来就有**的、语义上正确的替换点（我们替换的就是"PCGen 声明为可替换
> 的那个能力"），不需要新表。

### 3.4 验收

```bash
PCGEN_BIN=target/debug/pcgen python3 scripts/pf35e_extension_gate.py
# -> pf35e extension gate passed (4 variants layered on the RSRD,
#    64 classes unchanged, variants accepted/rejected by level-up)

# 人工排查某个变体为什么没生效：
PCGEN_BIN=target/debug/pcgen python3 -c "pass" # 或直接：
target/debug/pcgen list-variants --pcc <pcc> --class Fighter
# -> Weapon Master  ... replaces=1 grants=2  ok
#    （目标打不中时显示 UNRESOLVED(TYPE=...)）
```

Gate 分四层（详见脚本 docstring）：

1. **layering**：`game_mode == 35e`；`classes` 与 RSRD 单加载**完全相同**
   （变体层不加职业）；`abilities` 只增加 `pf35e_abilities.lst` 声明的条数；
   `variants` 等于变体文件声明的条数；叠加后
   `legacy_pending_by_tag` / `unsupported_pending_by_tag` 均为空。
2. **parsing**：每个 `TYPE:Archetype` 记录都进入 `CompiledRulePack.variants`；
   RSRD 单加载的 variant 数必须为 0（保证增量可归因）。
3. **behaviour**：`create` 写出的存档能被 `level-up --archetype` 接受并算出
   正确 BAB；未知 archetype id 被拒绝；`School Specialist` 用于 Fighter
   lane 时被拒绝；`evaluate` 能对带变体的 draft 产出 snapshot。
4. **semantics**：每个 `REPLACES` 目标都必须能命中该职业自己的特性行
   （用 `list-variants --format json` 的 `all_targets_resolved` 判定）。
   解析成功不等于生效——这一层就是用来抓"声明了但打不中"的变体。

> `evaluate` 子命令走的是内置 demo pack（不含真实规则包），所以**不能**用它
> 断言变体语义；语义层用 `list-variants` / `create` / `level-up` 这些带
> campaign 的命令。

### 3.5 数据自动化

`scripts/pf35e_extension_gate.py` 无 RSRD 时以退出码 2 **显式 SKIP**，
可与 `docs/TESTING.md` 的 no-data / full-data 分层对接。

### 3.6 覆盖度账目

变体记录的 `APPLIESTO` / `REPLACES` / `GRANTS` 在 lowering 阶段已被
`parse_variant` 消费，因此不计入 legacy pending；变体记录本身只进入
`CompiledRulePack.variants`，不重复计入 `abilities`（它是职业变体，
不是可选能力）。叠加后 RSRD 的 `semantic_completion` 保持 100%，
`legacy_pending_by_tag` 为空。

## 4. 有损之处（明确声明）

以下 PF 规则**无法**用 3.5e 语法无损表达，本扩展不做或只做近似：

| PF 特性 | 处理 | 影响 |
|---|---|---|
| CMB / CMD | 不表达 | 战斗数值缺这两项 |
| 1:1 技能点（跨职 1 点） | 用 3.5e `SKILLMULTIPLIER:4` 语义 → **不一致** | 技能点与 PF 原版不同 |
| favored class bonus | 不表达 | 每级可选奖励缺失 |
| archetype 的等级化**替换** | 替换是整条特性行级别，不带等级 | 不能表达"只替换该特性的某几个等级"；`REPLACES` 一旦命中就全程移除（这是 PF archetype 的常见语义，但不等价于所有情况） |
| bloodline / mystery / school 专有能力 | 描述性 ability，不实现数值 | 仅文本 |
| PF 专有职业（Alchemist 等） | 不重写 | 缺失（3.5e 无对应职业） |
| PF `.pcg` 存档 | 不兼容 | 需按 3.5e `.pcg` 另存 |

> **等级化 `GRANTS` 已支持**（不再是有损项）：`GRANTS:<feature>|PRECLASS:1,<Class>=<level>`
> 会在该职业等级达到后才把特性加入列表，见 §3.2。

**因此：本扩展产出的 PF 角色是"3.5e 化的近似卡"，不是 PF 官方的等价结果。**
任何以"PF 正确性"为标准的验收都不适用。

## 5. 与其它路线的关系

- **不替代** `P7` 的 `GameSystemProfile`：若将来要做**结果级** PF（CMB/CMD、
  1:1 技能点、favored class），正确路径是独立 profile（`P5` deferred 的原结论），
  而不是 3.5e 扩展。
- 本扩展是一个**务实的中间态**：让 PF 的 archetype 差异能进现有引擎、
  能编辑部组建卡，并诚实地把差距写在文档里。

## 6. 后续可选工作（按需）

1. 继续补充 PF 核心 archetype（Rogue / Barbarian / Bard / Druid / Sorcerer …）；
2. PF 专长/法术重写（工作量大，建议按 book 分批）；
3. 给扩展补独立 coverage 报告（复用 `import-report`）；
4. 若产品确认要结果级 PF，再回到 `P7` profile 路线。

## 7. 许可

Pathfinder 内容属 Paizo，依 OGL / Community Use Policy 使用；本扩展是
**独立重写**，不复制官方 PCGen Pathfinder 数据集文件。分发时需保留
`INFOTEXT` 中的声明。
