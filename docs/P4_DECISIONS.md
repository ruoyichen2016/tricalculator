# P4 范围决策记录

本文件记录 P4 roadmap 最后几个范围决策项的最终结论。结论以本文为准；
ROADMAP 中的历史描述保留为背景，但状态以本文件与 ROADMAP 顶部的
“P4 closure matrix” 为准。

## P4-DEC-1 KIT / CompanionMod / SIZE

### KIT / STARTPACK

- **P5-10 复审结论：维持 deferred。**
  - 当前主业务是重算已有 `.pcg` 角色，KIT runtime 不参与；
  - 启动需要固定 RNG seed、表语义、Java/PCGen 对照 fixture 与独立门禁；
  - 不启动不会阻塞 spell numeric / choice / pool 等 P5 工作。
- **P4 内范围**：声明的 KIT / STARTPACK typed import 与统计。
  - 已由 `P4-KIT-1` 完成。
  - coverage gate 保留 KIT typed counts。
- **P4 外 / deferred**：角色创建期运行时随机展开（`P4-KIT-2`）。
  - `OPTION` / `LOOKUP` / `VALUES` 权重选择、`QTY:roll(...)`、`EQUIPBUY`、
    `APPLY:INSTANT` 的随机/交互执行不在 P4 关闭范围内。
  - 重新评估条件：需要产品确认“创建期随机展开”是目标能力，并提供固定
    RNG seed、Java/PCGen 对照 fixture 和 acceptance case。
  - 当前不会因为不实现随机展开而回退 KIT typed import 或 coverage 基线。

### COMPANIONMOD

- **in scope，已完成**。
- `FOLLOWER:` 行级 `PRERACE` 选择、HD / SR / ABILITY / creature traits /
  `BONUS:AC` / `BONUS:STAT` 已进入 companion runtime。
- 真实 RSRD 108 行不再因同 id 覆盖丢失。

### SIZE

- **数值模型 in scope，已完成**：
  - `SIZENUM` 进入 `size_rank` / size modifier；
  - canonical size 顺序与 size 谓词稳定；
  - custom size UI/资料扩展属于 future scope，不影响 P4 关闭。

## P4-DEC-2 G3：XP 表 / point buy

- **状态：dropped / out-of-scope。**
- `XPCOST` 已进入 allowlist，不参与 `semantic_completion`。
- 旧 archive 的 XP 表 / point buy 测试意图标记为 dropped。

## P4-DEC-3 Pathfinder 1e

- **P5-10 复审结论：维持 deferred。**
  - 已完成技术试运行，见 `docs/P5_PATHFINDER_TRIAL.md`；
  - Pathfinder 专属 CMB/CMD、caster 公式、archetype/traits 仍缺失；
  - 继续放在正式 scope 之外。
- **状态：deferred / P4 外。**
- 当前 P4 只覆盖 D&D 3.0 / 3.5e RSRD。
- 重新纳入前置条件：
  1. 明确 Pathfinder 1e 数据来源与版本；
  2. 定义 profile 范围与首个 acceptance case；
  3. 新增 profile smoke 与 coverage 门禁；
  4. 重新评估 Pathfinder 特有如 CMB/CMD、traits、archetypes 等模型成本。
- 在满足上述条件前，不阻塞 P4 关闭。

## P5-10 TEMPVALUE UI

- **状态：维持 deferred。**
- `TEMPBONUS` / `TEMPVALUE` 已 typed 化，active-state 模型可用；
- `%CHOICE` 在 TEMPVALUE 选择值上的接入属于 UI / 交互扩展；
- 不阻塞当前 spell numeric / choice / pool 工作。

## P4-DEC-4 G4：旧 DataStore

- **状态：superseded。**
- G4 已由统一 Rule IR + `CompiledRulePack` 取代，无需单独实现。
- `ARCHIVE_TEST_MAP.md` 保持 no implementation needed。
