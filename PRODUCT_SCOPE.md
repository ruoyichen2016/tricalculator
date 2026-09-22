# PCGen-Rust 产品范围

## 目标

- D&D 3.0 / 3.5e 个人角色规则引擎。
- Pathfinder 1e：**正式延后（可复议）**；纳入前置条件见 `ROADMAP.md` P4-DEC-3。
- 玩家角色 1–20 级。
- 属性、种族、职业、专长、技能、豁免、BAB、AC、HP、装备、法术、灵能。
- Gestalt、双进阶职业、archetype/substitution、faith/divine lanes。
- PCGen `.pcc/.lst/.pcg` 导入与导出。
- CLI + Snapshot JSON + 文本/Markdown/HTML 输出。

## 明确包含

- 灵能：职业、威力、显能者等级、灵能点、威力 DC。
- Gestalt：并行 lane、best HP/BAB/豁免、技能并集、双进阶、slot cost。
- 信仰：deity/domains、多神术 lane、FaithPolicy。
- 迁移期保留 lowered / legacy / unsupported 统计。
- 数据集覆盖：3.0 SRD 与 3.5e RSRD 均在范围内；DS-3E 作回归代理，
  DS-35E 作主验收（见 `docs/TESTING.md` 第 7 节）。
- KIT / COMPANIONMOD 定义导入与统计（in scope，计入 DS-35E 分母）。
- 怪物记录（含 monster HD、natural attacks 等）计入范围与覆盖分母。

## 非目标

- 完整 PCGen Java 插件生态。
- 全部历史 tag、第三方资料、所有旧 `.pcg`。
- 旧 Swing/JavaFX GUI。
- 在运行时解析 `.lst`。
- 把派生值写回存档。
- 为了“兼容”而恢复多套并行规则模型。
- XP 表 / point buy 角色构建（G3，P4-DEC-2 结论：dropped）。
