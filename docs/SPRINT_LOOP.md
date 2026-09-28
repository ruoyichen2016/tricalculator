# Sprint Loop 执行状态

> 本文件是跨多轮对话的循环状态源。每轮执行一个 Loop Item：
> 实现 → 门禁 → 提交 → 更新本文件 → 报告后等待下一轮。

## 目标（按顺序）

- [x] **L1 Sprint 2 方案 A**：拆分 primary / extended cohort；主 cohort 恢复
      纯 3.0/3.5e，MSRD 进入 extended visibility，不参与主验收。
      - primary 25 case 全部可比字段 100%，0 owned known gaps。
      - extended 2 MSRD case 保留 7 条 owned known gaps，单独报告。
      - commit: `feat: split primary and extended broad cohorts`。
- [ ] **L2 Tri20 设计抽取 spike**：产出 `SystemProfile` + `SaveEnvelope`
      设计文档，不搬 Tri20 代码。
- [ ] **L3 Sprint 3 覆盖深度**：建立 `docs/COHORT_MATRIX.md`，扩展
      Java-gated 字段（技能总额 / 法术 DC / prepared / caster level /
      攻击加值 / 装备 AC 分量），并修复暴露的 engine 分歧。

## 循环规则

1. 每轮只做一个未完成 Item；前置 Item 未通过门禁前不开始下一个。
2. 每个 Item 的完成定义：代码/数据/文档落地 + `bash scripts/check.sh`
   通过 + 独立 commit。
3. 每轮结束更新本文件的勾选状态、commit、剩余风险与下一轮起始点。
4. 若门禁失败，优先修复；不得通过放宽 baseline 或删除断言换取通过。
5. 全部 Item 完成后循环结束。

## 当前状态

- 最近更新：L1 门禁全绿（coverage gate 15/15，archive differential 通过）。
- L1：已完成，待本轮 commit。
- L2：下一轮起点。
- L3：未开始。
