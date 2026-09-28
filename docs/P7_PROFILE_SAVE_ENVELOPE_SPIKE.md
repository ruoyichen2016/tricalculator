# P7 设计 spike：SystemProfile 与原生 SaveEnvelope

> 状态：**设计完成，未实现**。
> 来源参考：`reference/tri20-wsl-native` 的 `tri20-rules::SystemProfile` 与
> `tri20-storage::SaveEnvelope` 思路。本文只抽取设计，不复制 Tri20 代码，
> 不引入其 crate 结构。

## 1. 要解决的问题

当前 pcgen-rust 已有：

- `CompiledPackHeader.game_mode: Option<String>`（PCGen 的 `GAMEMODE` 字符串）；
- `CompiledPackHeader.rules_fingerprint` 与 `data_ref`；
- `CharacterDraft` 作为唯一输入事实源；
- `.pcg` 作为 PCGen 兼容存档格式；
- 3.0/3.5e 主验收，以及 Sprint 2 拆分出的 extended MSRD cohort。

缺口有两个：

1. **没有显式的“系统 profile”抽象。**
   `game_mode` 是字符串；如果将来要支持 MSRD / Pathfinder，engine 很容易
   长出按字符串分支的 `if game_mode == ...`，且 profile 语义无法被 fingerprint
   和门禁精确描述。

2. **没有原生、带版本与规则指纹的存档 envelope。**
   `.pcg` 是兼容格式，适合导入/导出，不适合作为编辑器/Web 的权威存档：
   它没有 schema 版本、rule-pack fingerprint、profile 与迁移策略。

## 2. SystemProfile 设计

### 2.1 放置位置

- 定义放在 `pcgen-core`（无文件依赖），例如 `pcgen-core/src/profile.rs`。
- 嵌入 `CompiledPackHeader`，字段名 `profile`：

```rust
pub struct CompiledPackHeader {
    pub game_mode: Option<String>,        // 保留为 PCGen provenance
    pub profile: GameSystemProfile,       // 新增：运行时语义 profile
    pub sources: Vec<String>,
    pub rules_fingerprint: String,
    pub data_ref: Option<String>,
}

pub struct GameSystemProfile {
    pub id: GameSystemId,       // D20_30 | D20_35 | PF1 | MSRD
    pub version: u32,           // profile schema/behavior version
    pub display_name: String,
    pub flags: ProfileFlags,    // 见 2.2
}
```

### 2.2 首批 flags（最小集）

```rust
pub struct ProfileFlags {
    pub class_defense_bonus: bool,       // d20 Modern / MSRD
    pub modern_save_progression: bool,   // Fast/Strong 等 MSRD 豁免表
    pub fractional_bab_saves: bool,      // PF1 optional rules
    pub cmb_cmd: bool,                   // PF1
    pub psionics: bool,                  // 3.5e RSRD / 3e
}
```

这些 flags 是**策略边界**，不是让 engine 到处写 `if`。正确用法是：

- engine 读取 `pack.header.profile.flags.xxx` 决定调用哪一个策略实现；
- 数值公式本身仍走统一 `Effect` / `Modifier` / `Predicate` IR；
- 不允许把 PCGen tag 分支写回 engine。

### 2.3 默认与迁移

- `GameSystemProfile::from_game_mode("35e") -> D20_35`、`"3e" -> D20_30`、
  `"Modern" / "MSRD" -> MSRD`，未知值给出诊断并保持当前行为。
- 第一阶段只做“默认 profile + 现有行为不变”：3.0/3.5e 的 flags 全为既有语义；
  `rules_fingerprint` 输入中加入 profile id/version/flags，确保 profile 变化
  能被 fingerprint 捕捉。
- MSRD / PF1：不在主 cohort 开启；先在 extended cohort 或独立 profile smoke
  中验证，再决定是否进入产品范围。

### 2.4 与现有字段的关系

| 现有字段 | 处理 |
|---|---|
| `game_mode: Option<String>` | 保留，作为 PCGen provenance；不再用于运行时判断 |
| `rules_fingerprint` | 输入加入 profile id/version/flags |
| `data_ref` | 保留；profile 不替代数据版本 |
| `pcgen-core::fingerprint::RuleFingerprintInput` | 增加 profile 字段，保持确定性 |
| `import-report` provenance | 新增 `profile` 输出；现有 `game_mode` 继续输出 |

## 3. 原生 SaveEnvelope 设计

### 3.1 目标

- 作为未来编辑器 / Web / 原生 CLI 的权威存档；
- 只保存输入，不保存派生值；
- 带 schema、应用版本、规则指纹、profile 与 data_ref；
- `.pcg` 仍是导入/导出兼容格式，二者不互相替代。

### 3.2 结构

```rust
pub struct NativeSaveEnvelope {
    pub schema: u32,                 // envelope schema
    pub application_version: String,
    pub profile: GameSystemProfileId,
    pub profile_version: u32,
    pub rules_fingerprint: String,
    pub data_ref: Option<String>,
    pub character: CharacterDraft,
}
```

### 3.3 加载规则

1. `schema` 高于当前支持 → 拒绝并给出明确错误；
2. `profile` 与当前 rule pack 不一致 → 结构化错误，不静默换算；
3. `rules_fingerprint` 不一致 → 允许加载但写诊断：
   - `warning: rule pack fingerprint changed: saved=... current=...`；
   - 不因为指纹不同就拒绝，避免阻塞数据升级。
4. `character` 必须是纯输入；BAB/HP/AC 等派生值不写入 envelope。

### 3.4 放置位置与实现顺序

- 先放在 **CLI / Web 应用边界**，不新增 crate：
  - `pcgen-cli` 新增 `save-native` / `evaluate-native`（或 `export --format native-json`）；
  - 序列化 `CharacterDraft` 即可，不需要 engine 知道 envelope。
- 等原生编辑器/多端同步成为真实需求后，再抽出 `pcgen-storage` crate。
- 不建议现在仿照 Tri20 直接建立 storage crate；当前 `.pcg` + CLI 已足够。

## 4. 与 Sprint 2 / Sprint 3 的关系

- **MSRD**：Sprint 2 方案 A 已把 MSRD 放入 extended cohort。
  如果将来走方案 B，应先实现 `ProfileFlags.class_defense_bonus` 与
  `modern_save_progression`，再给 extended cohort 建独立 gate。
- **PF1e**：同样先定义 profile，而不是把 PF1 case 塞进 3.0/3.5e 主 cohort。
- **Sprint 3 深度字段**：技能、法术、攻击等字段仍走统一 IR；
  profile 不改变字段门禁，只改变少数系统级策略。

## 5. 分阶段验收

| 阶段 | 内容 | 验收 |
|---|---|---|
| P7-P0 | 定义 `GameSystemProfile`，从 `game_mode` 派生默认值，写入 `CompiledPackHeader` 与 fingerprint | 所有现有测试与门禁行为不变；fingerprint 随 profile 变化 |
| P7-P1 | `import-report` / snapshot provenance 输出 profile | 3.0/3.5e 报告只增加字段，不改变数值 |
| P7-P2 | 原生 `NativeSaveEnvelope` + CLI 读写 | round-trip：draft → envelope → draft 完全一致；schema 拒绝测试 |
| P7-P3 | profile-specific 策略（MSRD / PF1，需产品确认） | 独立 cohort + 独立 acceptance gate；主 cohort 仍 100% |

## 6. 明确不做的事情

- 不复制 Tri20 的 `tri20-native-debug` 单体实现；
- 不引入 Tri20 的 crate 结构或 `tri20-storage`；
- 不让 engine 依赖 `.pcg` / JSON 文件格式；
- 不用 profile 替代 PCGen 数据版本 `data_ref`；
- 不在第一阶段改变 3.0/3.5e 任何派生数值。

## 7. 开放问题

1. `GameSystemId` 是否需要包含资料包维度，还是 profile 只描述规则语义？
2. `rules_fingerprint` 变更时，原生存档是否需要“迁移/重新绑定 pack”工具？
3. MSRD/PF1 的策略是 flags + 条件调用，还是独立 `ProfilePolicy` trait？
4. 原生 envelope 的首次落点是 CLI 还是 Web，还是两者共享一个小模块？
