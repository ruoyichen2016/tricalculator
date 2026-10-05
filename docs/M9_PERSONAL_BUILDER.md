# M9 个人建卡器范围（Personal Builder）

> 目标：自用 + 给普通朋友用，替代 PCGen 完成 D&D 3.0 / 3.5e 玩家角色建卡。
> 形态：本地 Web，打包数据，不做随机 KIT。

## 1. 已冻结的决策

| 决策 | 结论 |
|---|---|
| 购点 | **恢复**，作为建卡属性输入方式之一 |
| 规则系统 | **D&D 3.0 SRD + 3.5e RSRD** |
| 数据 | **打包**为可分发数据包，用户无需 fetch 全量测试数据 |
| 界面 | **Web 优先**，Windows 普通用户可一键启动 |
| 随机 KIT | **不做**；只做确定性起始装备包/预设 |

## 2. MVP 范围

### 包含

- 玩家角色 1–20 级；
- 种族、基础职业、多职业（逐级选择职业）；
- 格式塔：每个等级并行选择两个职业 lane；
- 属性：购点、手动输入、标准数组；
- 技能：技能点、跨职业技能（半点制）、最大 rank 校验（`角色等级+3`，跨职减半）；
- 专长：专长槽、先决条件；
- 法术：已知 / 准备 / 法术书（按职业）；
- 装备：起始装备、购买、负重；
- 神祇 / 领域；
- 升级：逐级升级与校验；
- 存档：原生 JSON envelope + 多角色管理；
- 输出：HTML 角色卡（浏览器打印）+ `.pcg` 导出；
- 数据包：3.0 SRD / 3.5e RSRD 玩家数据 + 版本信息。

### 不包含（MVP 外）

- Pathfinder 1e、MSRD / d20 Modern；
- 第三方资料书；
- 随机 KIT / 随机建卡表；
- 完整 PCGen 插件生态；
- 完整官方 output sheet 兼容；
- 云同步 / 多用户 / 在线服务；
- 桌面原生 GUI（先用本地 Web + 启动器）。

## 3. MVP 验收标准

1. Windows 用户解压后双击启动器，浏览器自动打开；
2. 可在 Web 中新建 1–20 级角色；
3. 至少支持 Fighter / Wizard / Cleric / Rogue 的完整建卡；
4. 购点、技能点（含跨职半点制与最大 rank 上限）、专长槽、法术位（含领域法术位）、装备预算能正确校验；
5. 角色可保存、打开、复制、删除；
6. 输出 HTML 角色卡可打印；
7. 可导入 / 导出 `.pcg`；
8. 核心数值与 PCGen csheet 对照一致；
9. 门禁保持全绿：`cargo test --workspace`（293）、`coverage_gate.py`
   15/15、deep-field 7 case / 49 gated 字段 / 0 缺口。验收基线以
   `ROADMAP.md`「当前状态」为准（本行数字随实现增长，不再复述固定值）；
10. 二次操作响应 < 1s（首次编译后可缓存）。

## 4. 架构落点

```text
pcgen-builder（新增，应用服务层）
   ├── NativeSaveEnvelope
   ├── CharacterRepository
   ├── point buy / creation workflow / level-up workflow
   └── validation

pcgen-web
   ├── 角色列表 / 创建向导 / 升级向导 / 角色卡打印
   └── 调用 pcgen-builder + pcgen-engine + pcgen-output

pcgen-data-pack（打包产物，不是 crate）
   └── 3.0 SRD / 3.5e RSRD 玩家数据 + pack.json 版本信息
```

依赖方向：

```text
pcgen-builder -> pcgen-engine -> pcgen-ir
pcgen-web     -> pcgen-builder
```

`pcgen-import` 仍只作为兼容导入层，不进入 builder/engine 的运行时依赖。

## 5. 数据包方案

数据包目录：

```text
data/packs/
  srd30/
    pack.json
    data/...
  rsrd35/
    pack.json
    data/...
```

`pack.json` 包含：

```json
{
  "id": "rsrd35",
  "name": "D&D 3.5e RSRD",
  "entry": "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
  "version": "1.0.0",
  "data_ref": "...",
  "license": "OGL-1.0a"
}
```

打包脚本只复制玩家建卡需要的 `.pcc/.lst` 子集，不打包 PCGen 测试套件。

## 6. 里程碑

| ID | 内容 | 状态 |
|---|---|---|
| M9-0 | 范围冻结 / 数据包方案 / 架构落点 | 已完成 |
| M9-1 | 原生存档 + 角色仓库 + Web 角色管理 | 已完成 |
| M9-2 | 购点 + create / level-up 规则闭环 | 已完成 |
| M9-3 | Web 创建向导 + 角色卡打印 | 已完成 |
| M9-4 | 数据包打包 + 启动器 | 已完成，已发布 0.1.0 |
| M9-5 | 朋友 beta 反馈与修复 | 进行中（M9-5a 已完成，M9-5b 待真实反馈） |
| M9-6 | 建卡深化：装备购买 / 法术书 / output sheet | 已完成 |
| M9-7 | 最终 release / beta 扩展（0.1.0） | 已完成，tag `v0.1.0-m9` |
| M9-8 | beta 反馈迭代 | 未开始，依赖 M9-5b |

## 7. 开放问题

1. 购点采用 3.5e DMG 标准表，还是需要自定义 budget / 3.0 表？
2. 起始装备采用“职业预设包”，还是完全自由商店购买？
3. XP 表是否进入 MVP？当前只要求恢复购点。
4. 数据包许可需要确认 OGL / PCGen 数据分发条款。
5. Web 启动器形态：纯 `.bat` / shell，还是 Rust 自带 `--open`？
