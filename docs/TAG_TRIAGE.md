# Tag backlog triage（阶段 0）

> 数据：DS-3E `srdbasic.pcc`（回归代理）与 DS-35E `_rsrd_basics.pcc`（主验收）。
> 口径：`legacy_pending + unsupported_pending`；本表覆盖当前全部 pending tag，
> 目标是**消除无主 backlog**（见 ROADMAP P0 / 审查发现 P0-2）。

## 0. 结论

- 本轮把 8 个纯展示/角色生成/UI tag 改判 allowlist，DS-35E pending 减少 125 次。
- 其余 74 个 pending tag **全部有了 owner**，其中 DS-35E 最大瓶颈是 KIT，
  单独新建 `P4-KIT-0`。
- 尚无任何 pending tag 被判定为 out-of-scope（除已 dropped 的 `XPCOST`），符合
  “KIT / COMPANIONMOD / 怪物 in scope”的决策。

## 0b. 进展（本文件是 triage 时的快照）

- `P4-PRE-3` **已实现**：`PRESPELLTYPE`/`PRESPELLBOOK`/`PRESPELLCAST` 已 lower；
- `P4-PRE-4` **已实现**：`PRETEMPLATE`/`PREHANDSGTEQ`/`PREHANDSEQ`/`PREMOVE` 已 lower；
- `P4-PRE-6` **部分实现**：size 谓词族已 lower，`PRETEXT` 改判 allowlist；
- 上述两行的剩余 tag：`DOMAINS`/`SPELLLEVEL`/`PROHIBITSPELL`/...（P4-SPELL-1）与
  `PRETEMPLATE`/`PREMOVE`/`!PRESUBCLASS`/`PRELANG`/`PREWEAPONPROF`（P4-TPL/EQP/RACE）。

## 1. 本轮改判 allowlist（已实施）

证据：这些 tag 仅出现在 `rsrd_biosettings.lst`（BIOSET）与 `rsrd_classes.lst`（CLASS `ROLE`），
描述角色外观/随机年龄或 UI 分组，不参与任何派生规则值。表中为该 tag 改判前的 pending 次数。

| tag | DS-3E | DS-35E | 位置 | 理由 |
|---|---:|---:|---|---|
| `AGEDIEROLL` | 0 | 28 | BIOSET | 随机起始年龄掷骰 |
| `BASEAGE` | 0 | 28 | BIOSET | 随机起始年龄基数 |
| `MAXAGE` | 0 | 28 | BIOSET | 随机年龄上限 |
| `HAIR` | 0 | 7 | BIOSET | 外观：发色 |
| `EYES` | 0 | 7 | BIOSET | 外观：瞳色 |
| `SKINTONE` | 0 | 7 | BIOSET | 外观：肤色 |
| `SEX` | 0 | 7 | BIOSET | 外观：性别随机表 |
| `ROLE` | 0 | 13 | CLASS | 职业 UI 角色（如 Combat） |

## 2. 语义 backlog → owner 映射（0 无主）

| owner | tag 数 | DS-3E | DS-35E | tags / 说明 |
|---|---:|---:|---:|---|
| **P4-KIT-0（新增设计 spike，DS-35E 主瓶颈）** | 6 | 75 | 6875 | `OPTION`, `LOOKUP`, `VALUES`, `EQUIPBUY`, `APPLY`, `QTY`<br/>KIT / STARTPACK 的随机选择权重（`OPTION:n,m`）、随机数量（`QTY:roll(...)`）、随机表（`LOOKUP`/`VALUES`）、起始装备预算（`EQUIPBUY:0`）、应用模式（`APPLY:INSTANT`）。KIT 已确认 in scope，故保留为语义 pending，不 allowlist；先产出设计 spike，判定声明式导入还是运行时随机展开，再决定实现路径。 |
| **P4-EQP-1（已有）** | 14 | 132 | 389 | `BASEITEM`, `SLOTS`, `LOCATION`, `CHARGES`, `CONTAINS`, `BASEQTY`, `ALTCRITMULT`, `ALTCRITRANGE`, `ALTDAMAGE`, `ALTEQMOD`, `ALTTYPE`, `ARMORTYPE`, `CHANGEPROF`, `WEAPONBONUS`<br/>装备基础物品、槽位/位置、充能、容器、基础数量与武器/护甲变体；对应 G2。 |
| **P4-TEMP-1（新增）** | 2 | 189 | 376 | `TEMPBONUS`, `TEMPVALUE`<br/>临时加值/临时值（如 Rage、Bardic Inspire），需要 active-state 模型，不是静态 snapshot。 |
| **P4-PRE-3 / P4-SPELL-1（已有 + 新增）** | 13 | 236 | 407 | `DOMAINS`, `SPELLLEVEL`, `PROHIBITSPELL`, `ADDSPELLLEVEL`, `KNOWNSPELLSFROMSPECIALTY`, `SPELLBOOK`, `PRESPELLTYPE`, `PRESPELLBOOK`, `PROHIBITCOST`, `ADDDOMAINS`, `PRESPELLSCHOOL`, `PRESPELLCAST`, `PRESPELL`<br/>法术类型/法术书谓词（PRESPELL*）、领域、法术等级、禁止法术、额外法术位与专精已知法术。 |
| **P4-RACE-1（新增，含生物特征）** | 13 | 579 | 838 | `RACESUBTYPE`, `REACH`, `REACHMULT`, `NATURALATTACKS`, `UDAM`, `UMULT`, `DR`, `SR`, `CR`, `VISION`, `MOVECLONE`, `UNENCUMBEREDMOVE`, `ATTACKCYCLE`<br/>种族子类、触及、天武、徒手伤害、DR/SR/CR、视觉与移动克隆；怪物记录已确认 in scope。 |
| **P4-CLASS-1（新增）** | 7 | 89 | 29 | `SKILLMULT`, `LEVELSPERFEAT`, `HITDIE`, `EXCLASS`, `ADDLEVEL`, `EXCHANGELEVEL`, `XTRASKILLPTSPERLVL`<br/>技能点倍率、每级专长数、生命骰、进阶职业退出/交换等级与额外技能点。 |
| **P4-STAT-1（新增）** | 3 | 54 | 46 | `DEFINESTAT`, `STATMOD`, `SIZENUM`<br/>自定义属性定义与边界（`DEFINESTAT:MINVALUE`）、属性调整、尺寸序号。 |
| **P4-PRE-6（新增，谓词补齐）** | 14 | 16 | 244 | `BONUS`, `PREMULT`, `PRETEMPLATE`, `!PRETEMPLATE`, `PRESIZEEQ`, `PRESIZEGTEQ`, `PREHANDSGTEQ`, `PREHANDSEQ`, `PREMOVE`, `!PREMOVE`, `!PRESUBCLASS`, `PRETEXT`, `PRELANG`, `PREWEAPONPROF`<br/>仍未 lower 的谓词：残余 BONUS/PREMULT、模板/尺寸/手/移动/语言/武器熟练，以及正向形式未支持的 `!PRETEMPLATE`/`!PREMOVE`/`!PRESUBCLASS`（等正向实现后自动受益）。 |
| **P4-BONUS-1（已有）** | 6 | 134 | 299 | `STACK`, `VARIANTS`, `ABILITYLIST`, `CHOICE`, `CHOOSE`, `POOL`<br/>加值叠加键、能力变体/清单/选择与池。 |
| **P4-FAITH（新增小项）** | 3 | 5 | 5 | `DEITYWEAP`, `VALIDFORDEITY`, `VALIDFORFOLLOWER`<br/>神祇偏好武器与信仰校验。 |
| **P4-DEC-2（已 dropped）** | 1 | 5 | 10 | `XPCOST`<br/>XP 成本，属 G3 dropped 范围，保留记录但不再实现。 |
| *allowlist（本轮）* | 8 | 0 | 0 | `AGEDIEROLL`, `BASEAGE`, `MAXAGE`, `HAIR`, `EYES`, `SKINTONE`, `SEX`, `ROLE` |
| **合计** | **82** | **1514** | **9518** | |

## 3. 需要 category-aware 规则的 tag

当前 `tagclass::classify(tag)` 只按 tag 名分桶，但下列 tag **同名不同义**，
triage/实现时必须按 category 区分，否则会误 allowlist 掉语义：

| tag | KIT/STARTPACK 侧 | 其它 category 侧 |
|---|---|---|
| `APPLY` | `APPLY:INSTANT`（428，kit 应用模式）→ `P4-KIT-0` | `APPLY:Shield\|PREEQUIP`（equipmod）→ `P4-EQP-1` |
| `QTY` | `QTY:roll(\"1d6\")*1000`（kit 内随机数量）→ `P4-KIT-0` | 装备堆叠/数量 → `P4-EQP-1` |
| `EQUIPBUY` | `EQUIPBUY:0`（STARTPACK）→ `P4-KIT-0` | 潜在装备购买预算 → `P4-EQP-1` |

建议在设计 `P4-KIT-0`、实现 `P4-EQP-1` 时把 `classify` 升级为
`classify(category, tag)`，并保留 tag-only 作为兜底。

## 4. Gap D：known-but-unbucketed（待补门禁）

`is_known_tag()` 为真的 tag 若既未 lower、也未进入 `consumed_legacy_tag`，
会**既不进 pending 也不进 allowlist**（例如 `VISIBLE`）。这意味着
`semantic_completion` 的分母漏掉了一部分已知 tag。建议：
- 把 `VISIBLE` 等纯展示 tag 显式 allowlist；
- 给覆盖度增加一个 `known_consumed` / `known_ignored` 桶，使
  `lowered + consumed + allowlisted + pending` 覆盖所有 known tag。

## 5. 后续动作

1. `P4-KIT-0` 设计 spike 已完成（见 [`P4_KIT0_DESIGN.md`](P4_KIT0_DESIGN.md)）：声明式 typed 导入优先，运行时随机展开单独立项；下一步 P4-KIT-1。
2. `P4-PRE-6` 与 `P4-PRE-3` 并行：先清谓词类 pending。
3. 实现 `classify(category, tag)` + Gap D 的 known 桶，随后一次性重新冻结 coverage baseline。
