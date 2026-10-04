# PF 1e 作为 3.5e 扩展（重写）— 设计与范围

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

### 3.1 文件

```text
extensions/pf35e/
  rsrd_pf35e.pcc.in          # 模板（RSRD 路径由脚本注入）
  data/pf35e_classes.lst     # Cleric / Fighter / Wizard
  data/pf35e_abilities.lst   # Channel Energy 等描述性 class feature
```

`scripts/package_pf35e_extension.py` 把扩展复制到
`<pcgen-data>/data/_pf35e_extension/`，生成可加载的 `rsrd_pf35e.pcc`。

### 3.2 当前覆盖

| 职业 | HD | BAB 公式 | 豁免 | 施法 |
|---|---|---|---|---|
| Cleric | d8 | `classlevel()/2+2` | 强 Fort/Will | WIS，准备施法，CAST 表 1–20 |
| Fighter | d10 | `classlevel()` | 强 Fort | — |
| Wizard | d6 | `classlevel()/2` | 强 Will | INT，准备施法，CAST 表 1–20 |

这些公式**与 3.5e 对应职业一致**——PF 在这几项上没有改规则，因此可无损表达。

### 3.3 验收

```bash
PCGEN_BIN=target/debug/pcgen python3 scripts/pf35e_extension_gate.py
# -> pf35e extension gate passed (3 classes, BAB/saves match PF formulas)
```

Gate 校验：
- 加载后 `game_mode == 35e`；
- 职业数 ≥ 92（RSRD 89 + 扩展 3）；
- Cleric3 / Fighter3 / Wizard3 的 BAB 与三项豁免符合 PF 公式。

### 3.4 数据自动化

`scripts/pf35e_extension_gate.py` 无 RSRD 时以退出码 2 **显式 SKIP**，
可与 `docs/TESTING.md` 的 no-data / full-data 分层对接。

## 4. 有损之处（明确声明）

以下 PF 规则**无法**用 3.5e 语法无损表达，本扩展不做或只做近似：

| PF 特性 | 处理 | 影响 |
|---|---|---|
| CMB / CMD | 不表达 | 战斗数值缺这两项 |
| 1:1 技能点（跨职 1 点） | 用 3.5e `SKILLMULTIPLIER:4` 语义 → **不一致** | 技能点与 PF 原版不同 |
| favored class bonus | 不表达 | 每级可选奖励缺失 |
| archetype / traits | 部分可用已有 variant 机制近似 | 替换语义不完全等价 |
| bloodline / mystery / school 专有能力 | 描述性 ability，不实现数值 | 仅文本 |
| PF 专有职业（Alchemist 等） | 暂未重写 | 缺失 |
| PF `.pcg` 存档 | 不兼容 | 需按 3.5e `.pcg` 另存 |

**因此：本扩展产出的 PF 角色是"3.5e 化的近似卡"，不是 PF 官方的等价结果。**
任何以"PF 正确性"为标准的验收都不适用。

## 5. 与其它路线的关系

- **不替代** `P7` 的 `GameSystemProfile`：若将来要做**结果级** PF（CMB/CMD、
  1:1 技能点、favored class），正确路径是独立 profile（`P5` deferred 的原结论），
  而不是 3.5e 扩展。
- 本扩展是一个**务实的中间态**：让 PF 内容能进现有引擎、能编辑部组建卡，
  并诚实地把差距写在文档里。

## 6. 后续可选工作（按需）

1. 继续重写更多 PF 核心职业（Rogue / Barbarian / Bard / Druid / Sorcerer …）；
2. PF 专长/法术重写（工作量大，建议按 book 分批）；
3. 给扩展补独立 coverage 报告（复用 `import-report`）；
4. 若产品确认要结果级 PF，再回到 `P7` profile 路线。

## 7. 许可

Pathfinder 内容属 Paizo，依 OGL / Community Use Policy 使用；本扩展是
**独立重写**，不复制官方 PCGen Pathfinder 数据集文件。分发时需保留
`INFOTEXT` 中的声明。
