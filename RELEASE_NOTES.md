# PCGen-Rust Web 0.1.0 (M9 Beta)

发布日期：2026-09-29（开发快照）

## 这是什么

PCGen-Rust Web 是一个面向 D&D 3.0 / 3.5e 的本地角色建卡器：

- 导入已有 PCGen `.pcg` 角色并重算；
- Web 建卡向导：购点、种族、职业、技能、专长、法术、装备；
- 原生角色存档与多角色管理；
- HTML 角色卡打印；
- 导出 `.pcg` 供 PCGen 使用。

## 运行方式

Windows：

1. 解压发布包；
2. 双击 `run-web.bat`；
3. 浏览器打开 `http://127.0.0.1:8787`。

Linux / macOS：

```bash
./run-web.sh
```

## 随包内容

- `pcgen-web` release 二进制；
- `data/packs/rsrd35`：D&D 3.5e RSRD 玩家数据；
- `data/packs/srd30`：D&D 3.0 SRD 玩家数据；
- `characters/`：原生角色存档目录；
- `run-web.sh` / `run-web.bat`；
- `M9_PACKAGING.md` / `M9_BETA_CHECKLIST.md`。

## 功能状态

- ✅ 3.0 / 3.5e 基础职业与多职业导入；
- ✅ 购点、技能、专长、法术、装备预算；
- ✅ 原生角色保存 / 打开 / 删除；
- ✅ HTML 角色卡与打印；
- ✅ `.pcg` 导入 / 导出；
- ⚠️ 装备商店与官方 output sheet 完整兼容仍在收敛；
- ❌ 不支持 Pathfinder / Modern / 随机 KIT。

## 质量证据

> 以下数字描述 **2026-09-29 的 0.1.0 冻结快照**，不代表当前 HEAD。
> 当前状态以 [`ROADMAP.md`](ROADMAP.md)「当前状态」一节为准。

- `cargo test --workspace`：227 passed；
- `coverage_gate.py`：15 pass / 0 fail / 0 skip；
- primary broad cohort：25/25 Java-gated；
- deep-field oracle：7 cases / 49 gated / 0 owned gaps；
- archive 三方差分：8 cases / 0 regression / 0 divergence。

## 已知限制

- 首次加载 `.pcc` 需要数秒编译规则包；
- PCGen 数据许可与分发需遵守 OGL / PCGen 条款；
- 不支持随机 KIT；
- 复杂第三方资料与旧版 `.pcg` 未完全验证；
- 浏览器打印质量取决于浏览器 PDF 导出设置。

## 反馈

请按 `M9_BETA_CHECKLIST.md` 试用，并点 Web 页面 **Copy diagnostics**
把 snapshot 与 diagnostics 一起反馈。
