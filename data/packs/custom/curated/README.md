# Curated Data Packs

这里存放**已经由我们审定/校正后的资料包**。

- 每个资料包都会经过导入检查、语法修正、字段校验；
- 可用于 Web 建卡；
- 启动时优先使用这里的资料。

建议结构：

```text
curated/
  my_pack/
    pack.json
    data/
      ...
```

启动方式：

```bash
PCGEN_DATA_ROOT="$PWD/data/packs/custom/curated/my_pack" ./run-web.sh
```

## 当前已审定资料

- `bahamut-all-35e3e/`：3e/35e 全量扩展聚合包（默认载入）。
  - 覆盖原始仓库全部 3e/35e 资料：**3e 13 个 PCC + 35e 102 个 PCC = 115 个 PCC**；
  - 已修正编码：8 个非 UTF-8 文件转为 UTF-8；
  - 已修正大小写引用：7 处 PCC 文件引用与磁盘文件名不一致；
  - 已补全 `_universal/races.lst` 共享依赖；
  - 入口 `data/_all_35e3e.pcc`，`error_diagnostics=0`。
  - 包含全部 3e、35e 的 `.pcc`；
  - 已修正编码（UTF-8）与文件名大小写引用；
  - 入口 `data/_all_35e3e.pcc`；
  - `variants=96`，`error_diagnostics=0`。
- `bahamut-all-35e3e/` 与 `bahamut35e-wotc-supplements/` 已补充：
  - **Lightbringer Cleric**：Cleric 1 级替代等级；
  - 包含 Destroy Undead / True Daylight / Positive Healing 三个替代能力。
- `bahamut35e-wotc-supplements/`：3.5e WotC 扩展集合。
  - 包含 Core + 全部 WotC supplement；
  - 入口 `data/35e/wotc_supplements.pcc`；
  - 已通过导入检查。
- `bahamut3e-core/`：3e 核心选集。
  - 入口 `data/3e/bahamut3e_core.pcc`；
  - 已通过导入检查。
- `bahamut35e-players/`：面向玩家建卡的优化资料包。
  - 只抽取 Player's Handbook、Universal Rules、Unearthed Arcana；
  - 不包含 DMG / MM / 美术资源 / outputsheets 等非玩家必需内容；
  - 自建精简入口 `data/35e/bahamut_players.pcc`；
  - 已通过 `pcgen import-report` 导入检查，`error_diagnostics=0`。
