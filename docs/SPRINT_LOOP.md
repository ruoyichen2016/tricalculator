# Sprint Loop 执行状态

> 本文件是跨多轮对话的循环状态源。每轮执行一个 Loop Item：
> 实现 → 门禁 → 提交 → 更新本文件 → 报告后等待下一轮。

## 目标（按顺序）

- [x] **L1 Sprint 2 方案 A**：拆分 primary / extended cohort；主 cohort 恢复
      纯 3.0/3.5e，MSRD 进入 extended visibility，不参与主验收。
      - primary 25 case 全部可比字段 100%，0 owned known gaps。
      - extended 2 MSRD case 保留 7 条 owned known gaps，单独报告。
      - commit: `feat: split primary and extended broad cohorts`。
- [x] **L2 Tri20 设计抽取 spike**：产出 `SystemProfile` + `SaveEnvelope`
      设计文档，不搬 Tri20 代码。
      - 产出 `docs/P7_PROFILE_SAVE_ENVELOPE_SPIKE.md`。
      - 挂接现有 `CompiledPackHeader` / `rules_fingerprint` / `CharacterDraft`。
      - check.sh 全绿；无代码行为变化。
- [~] **L3 Sprint 3 覆盖深度**
      - [x] **L3a COHORT_MATRIX + 未纳入裁决**：42 PCG / 32 XML / 18 HTM
        全部归类；companion / legacy / out-of-scope 均有裁决；
        `scripts/cohort_matrix.py --check` 接入 full-data CI。
      - [x] **L3b-1 深度字段门禁**：7 case / 24 gated fields /
        14 owned known_gaps；新增 `deep_field_oracle.py`、
        `docs/DEEP_FIELDS.md`，攻击总额先作为 visibility。
      - [x] **L3b-2 深度字段归一化修复**：equipment_ac_bonus / skill_total /
        spell_dc_max / caster_level_max 完成归一化，deep-field 收敛到
        23 gated / 5 owned gaps。
      - [x] **L3b-3 Domain caster level**：新增
        `CharacterSnapshot.caster_level_bonus`，修掉 `3e_CleElf`；deep-field
        收敛到 24 gated / 4 owned gaps。
      - [ ] **L3b-4 残留修复**：区分 known / memorized / innate 法术语义，
        暴露 Rust melee/ranged/grapple 总额，消除剩余 4 条 known_gap。

## 循环规则

1. 每轮只做一个未完成 Item；前置 Item 未通过门禁前不开始下一个。
2. 每个 Item 的完成定义：代码/数据/文档落地 + `bash scripts/check.sh`
   通过 + 独立 commit。
3. 每轮结束更新本文件的勾选状态、commit、剩余风险与下一轮起始点。
4. 若门禁失败，优先修复；不得通过放宽 baseline 或删除断言换取通过。
5. 全部 Item 完成后循环结束。

## 当前状态

- 最近更新：L3b-2 完成（commit `d93bb15`）。
- L1：已完成（commit 见 git log）。
- L2：已完成。
- L3a：已完成（commit `5193835`）。
- L3b-1：已完成（commit `d010547`）。
- L3b-2：已完成（commit `d93bb15`）。
- L3b-3：已完成，待本轮 commit。
- L3b-4：下一轮起点。
