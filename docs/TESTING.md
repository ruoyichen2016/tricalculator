# 测试与门禁

本文档说明 PCGen-Rust 的四层测试/门禁，以及固定测试数据的来源与升级流程。
新增或修改门禁时，请同步更新这里和 `.github/workflows/ci.yml`。

## 1. 四层证据

| 层 | 命令 | 需要 `raw/pcgen-master` | 运行时机 |
|---|---|---|---|
| no-data | `cargo test --workspace` + `python3 scripts/coverage_gate.py --allow-skip` | 否 | push / pull_request |
| full-data | `scripts/fetch_testdata.sh` → `scripts/check_testdata.sh` → `cargo test` → `python3 scripts/coverage_gate.py` | 是 | nightly / `workflow_dispatch` |
| broad oracle | `python3 scripts/broad_oracle.py --check [--json]` | 是 + 已冻结 baseline | nightly（由 coverage gate 调用一次） |
| archive differential | `python3 scripts/snapshot_diff_archive.py --strict` | 是 + 旧 archive 二进制 | nightly / 手动（含在 full-data job） |

CI 的 `no-data` job 会在缺少外部数据时**显式**打印 `SKIP`，但因为它使用的是
`--allow-skip`，所以只作为 smoke，不代表真实数据通过。`full-data` job 先执行
`check_testdata.sh`，再用默认严格模式运行 `coverage_gate.py`；只要任何
data-dependent gate 被跳过，就会以退出码 2 失败。

## 2. 固定测试数据

数据来源与校验全部由**单一文件** `scripts/testdata.lock.json` 定义（repo、
ref、tag、tarball URL、sparse paths、必需文件 + blob hash + size、最小文件数）。
升级数据版本只需要改这一个文件，`fetch` / `check` 脚本不再各自维护路径和 hash。

当前锁定：

```text
repo: PCGen/pcgen
tag:  v6.09.08.RC1-NIGHTLY.20260911
ref:  f91bd03f2dbe1145d5526b52aa3e48fcd0f2fc8d
```

`scripts/fetch_testdata.sh` 读取该 lock，从 codeload 下载对应 tarball，只解出
`sparse_paths`，写入 `<workspace>/raw/pcgen-master/`：

```text
data/                     # 3e / 35e / killshot / zen_test 等 campaign
code/testsuite/           # PCGfiles + csheets（Java differential 基线）
outputsheets/             # 官方 FreeMarker sheet fixture
gradle.properties         # 版本与 provenance 参考
.pcgen-rust-testdata      # 由 fetch 脚本写出的 marker（repo/ref/时间）
```

`scripts/check_testdata.sh` 同样读取 lock，校验必需文件、最小文件数，并用
`git hash-object`（无 git 时退回 Python SHA-1）对照 12 个已知 blob 的
SHA-1，防止“目录存在但来自其它 revision”的假通过。当前固定数据在本地与上游
commit 的 blob hash 完全一致。

## 3. 常用命令

```bash
# 只跑仓库内测试 + mini 门禁（无外部数据也可通过）
CARGO_HOME="$PWD/.cargo-home" cargo test --workspace
python3 scripts/coverage_gate.py --allow-skip

# 抓取固定数据并跑完整门禁
bash scripts/fetch_testdata.sh
bash scripts/check_testdata.sh
python3 scripts/coverage_gate.py          # 严格模式；skip 即失败（退出码 2）

# 完整开发检查
bash scripts/check.sh
```

退出码约定：

| code | 含义 |
|---|---|
| 0 | 通过（若使用 `--allow-skip`，表示 no-data smoke 通过） |
| 1 | 有门禁断言失败 |
| 2 | 缺少 fixed 数据导致门禁被跳过（严格模式） |

## 4. 升级数据版本

1. 只修改 `scripts/testdata.lock.json`：更新 `repo` / `ref` / `tag` /
   `tarball_url` / `sparse_paths` / `required_files`（含 blob hash + size）与
   `minimum_counts`。`fetch_testdata.sh` / `check_testdata.sh` 都读取该 lock，
   不再各自维护路径与 hash。
2. 运行 `bash scripts/fetch_testdata.sh --dest /tmp/pcgen-new`，再核对
   `bash scripts/check_testdata.sh /tmp/pcgen-new`。
3. 用新数据跑 full-data 门禁；如果 Java differential 的 golden 变化，
   在同一个变更里更新统计基线和说明，而不是放宽阈值。
4. 如果 `tests/fixtures/differential/broad_cohort.json` 的 `data_ref` 与 lock
   不一致，按第 6 节重新生成 broad baseline。
5. 在 PR / 变更记录里写出旧 ref → 新 ref、数据版本，以及受影响的 case。

## 5. archive differential

`scripts/snapshot_diff_archive.py` 用旧的 archive 二进制作为冻结 oracle，逐字段
比较 archive v1 与新 engine 的 snapshot。默认二进制路径为
`reference/pcgen-rust-archive/target/debug/pcgen`，可用 `ARCHIVE_PCGEN` 覆盖：

```bash
(cd ../reference/pcgen-rust-archive && cargo build)   # 仅首次/变更后
python3 scripts/snapshot_diff_archive.py --strict
```

它是 **三方差分**：archive vs new vs Java csheet，由 Java 裁决：

- `ARCHIVE-BUG`：archive 与 new 不一致，但 new == java（archive 自身有 bug）
- `REGRESSION`：archive == java 而 new 不一致（新引擎回归）→ 失败
- `DIVERGENCE`：三者都不一致 → 失败

已接入 CI `full-data` job。退出码 1 仅在出现 REGRESSION / DIVERGENCE 时返回。

## 6. broad oracle

`scripts/differential_java_pcgen_all.py` 是可见性报告（恒退出 0）。
`scripts/broad_oracle.py` 把它升级为真正的 per-case 门禁：

- `tests/fixtures/differential/broad_cohort.json` 固定 cohort（case 列表 +
  `java_gated_fields`）与 `data_ref`；
- `tests/fixtures/differential/broad_baseline.json` 冻结当前观测值
  （含已知差异），任何字段变化都会失败；
- cohort 中标记 `java_gated_fields` 的 case，还必须与 Java csheet 精确一致；
- cohort 中标记 `expectation = computed_diverging` 的 case 必须能计算，但不要求
  与 Java 一致（数值仍冻结在 baseline，用于捕捉漂移）；它们不计入 java 一致率分母。

```bash
python3 scripts/broad_oracle.py --write-baseline   # 冻结当前行为（会改动 baseline）
python3 scripts/broad_oracle.py --check            # 门禁
python3 scripts/broad_oracle.py --check --json     # 结构化摘要（coverage gate 使用）
python3 scripts/broad_oracle.py --report           # 字段一致率报告
```

`--check --json` 输出结构化摘要，把口径拆成：

- `actual.computed`：必须计算且实际算出的 case 数（含 known-diverging）；
- `actual.java_compared`：其中与 Java 一致率分母相同的 case 数；
- `actual.known_diverging`：声明为 `computed_diverging` 的 case 数；
- `actual.data_missing` / `actual.java_gated`；
- `expected`：cohort 声明的 `computed` / `computed_diverging` / `must_compute` /
  `data_missing_limit` 等。

`coverage_gate.py` 直接读取该 JSON 做分母守卫，不再解析人类可读文本。
`broad_oracle.py` 在 full-data 流程中只由 coverage gate 调用一次；
`scripts/check.sh` 和 CI 不再单独重复调用。`differential_java_pcgen_all.py`
仍然是独立的 visibility report，不参与 per-case 门禁。

所有门禁脚本通过 `scripts/pcgen_cli.py` 构建一次 CLI，并直接执行
`target/debug/pcgen`，不再对每个 case 启动一次 `cargo run`。

升级数据版本时，`broad_cohort.json` 的 `data_ref` 必须与 `testdata.lock.json`
一致，否则 `--check` 会直接失败提示重新生成 baseline。

## 7. tag 直方图与工作清单

`import-report` 现在输出 `coverage.unsupported_by_tag` / `legacy_by_tag`：

```bash
cargo run -q -p pcgen-cli -- import-report <pcc> --json
cargo run -q -p pcgen-cli -- import-report <pcc> --histogram --top 40
```

`import-report` 输出三类直方图：

- `legacy_allowlisted_by_tag`：展示 / 元数据 / 暂不处理的 tag，**不计入分母**
- `legacy_pending_by_tag`：真正待迁移的语义 tag
- `unsupported_pending_by_tag` / `unsupported_allowlisted_by_tag`：
  lowering 拒绝的 tag 也按同一规则分桶

分类由 `pcgen-core/src/tagclass.rs` 的 `ALLOWLISTED_TAGS` 决定，未列入者
一律视为待迁移。门禁断言两套划分都精确：
`legacy_allowlisted + legacy_pending == legacy_tags`、
`unsupported_allowlisted + unsupported_pending == unsupported_tags`，
并打印
`semantic_completion = lowered / (lowered + legacy_pending + unsupported_pending)`。

### 7.1 验收数据集

产品范围覆盖 3.0 / 3.5e，单一数据集不足以代表完成度，因此固定两个口径：

| 数据集 | 入口 | 角色 |
|---|---|---|
| **DS-3E** | `data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc` | 回归代理（变化快、便于定位） |
| **DS-35E** | `data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc` | **主验收**（3.5e 主数据集） |
| DS-FULL | `_revised_system_reference_document.pcc` | 信息性全量报告，不设门禁 |

KIT / COMPANIONMOD / 怪物记录**均在范围内**（见 `PRODUCT_SCOPE.md`），
因此 DS-35E 分母不排除这些 category；`OPTION` / `LOOKUP` 等若判定为
展示 / 编辑器元数据，应通过 `tagclass.rs` 的 allowlist 显式改判，
而不是调分母。

### 7.2 coverage 非回归基线

`tests/fixtures/coverage/coverage_baseline.json` 冻结两个数据集的关键计数与
**逐 tag 的 pending 次数**。门禁（`coverage_gate.py`）对每个数据集断言：

- `entities` / `tag_occurrences` / `lowered_effects` 不得低于基线；
- `semantic_pending` 不得高于基线，`semantic_completion` 不得低于基线；
- **任何一个 tag 的 pending 次数不得增加**。

允许的改动方向是 lower / allowlist / 桶间改判（legacy ↔ unsupported）；
只有经过评审、有意接受的回归才可以用
`python3 scripts/coverage_gate.py --write-coverage-baseline` 重新冻结基线。
数据版本升级时 baseline 的 `data_ref` 必须与
`scripts/testdata.lock.json` 一致，否则门禁打印告警。

`docs/TAG_WORKLIST.md` 由 DS-3E 生成，把
`unsupported=3`、`legacy_allowlisted=1,284`、`legacy_pending=1,489`
拆成可分配的 tag 项（`semantic_pending = 1,492`）。

每个 pending tag 的 owner 划分见 [`docs/TAG_TRIAGE.md`](TAG_TRIAGE.md)：
8 个展示/角色生成 tag 已改判 allowlist，其余全部归入 P4 任务；
DS-35E 的 KIT 主瓶颈已由 `P4-KIT-1` 声明式 typed 导入解决；
设计结论见 [`P4_KIT0_DESIGN.md`](P4_KIT0_DESIGN.md)：声明式导入优先，运行时
随机展开单独立项。当前 DS-35E `semantic_completion` 约 **88.3%**，
DS-3E 约 **84.2%**；两个数据集 unsupported pending 均为 0。

## 8. 架构护栏：engine 不得依赖 import

迁移已把运行时 IR 提到 `pcgen-ir`：

```text
pcgen-import  ->  pcgen-ir  <-  pcgen-engine
```

`pcgen-engine` 的生产依赖只有 `pcgen-core` / `pcgen-formula` / `pcgen-ir`
（+ 第三方）。`pcgen-import` 仅作为 `dev-dependency` 供
`snapshot_summary.rs` 这类 loader+engine 集成测试使用。

`scripts/check.sh` 会运行 `scripts/check_architecture.sh`；CI 的 `no-data`
job 也执行它。检查内容：

1. `pcgen-engine` / `pcgen-ir` 的生产依赖树不含 `pcgen-import`
2. `pcgen-engine` / `pcgen-import` 的生产依赖树含 `pcgen-ir`
3. engine 源码不出现 `"RACE"` / `"CLASS_LEVEL"` / `"EQUIPMENT"` / `"PCSTAT"`
   等原始 category 字符串分支（应使用 `pcgen_core::EntityCategory` /
   `ChoiceKind`）
4. `PcgDraft` 数据类型的定义位于 `pcgen-ir`，而非 `pcgen-import`
