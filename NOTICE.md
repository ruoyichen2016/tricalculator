# NOTICE

本仓库包含三类不同许可的内容，**不要混为一谈**：

| 范围 | 内容 | 许可 |
|---|---|---|
| 本仓库源码 | `crates/`、`scripts/`、文档 | **LGPL-2.1**（见 [`LICENSE`](LICENSE)） |
| 游戏数据（第三方） | `raw/pcgen-master/` 固定快照、`data/packs/` 数据包 | **OGL 1.0a** + PCGen 数据条款（见下） |
| PF 变体扩展 | `extensions/pf35e/`（含 `.lst` 数据） | 本项目独立重写；Paizo 内容依 OGL / Community Use |

> `extensions/pf35e/` 既有生成脚本读入的模板，也有 `.lst` **数据**文件。
> 数据部分按 Paizo 的 OGL / Community Use 处理，不适用 LGPL。

---

## 1. 本仓库源码 — LGPL-2.1

`crates/`、`scripts/` 与文档由本项目以
**GNU Lesser General Public License v2.1** 发布，全文见 [`LICENSE`](LICENSE)。
这与上游 PCGen 代码的许可一致（`Cargo.toml` 声明 `license = "LGPL-2.1"`）。

## 2. 游戏数据 — OGL 1.0a

`raw/pcgen-master/` 是 **PCGen 项目**的固定版本快照
（`PCGen/pcgen` @ `f91bd03f2dbe1145d5526b52aa3e48fcd0f2fc8d`，
tag `v6.09.08.RC1-NIGHTLY.20260911`），
`data/packs/` 下的数据包由这些数据打包而来。
二者**不是**本项目的 LGPL 代码，其使用受 **Open Game License v1.0a** 约束。

`d20 System` 与 `System Reference Document` 的 OGL 声明要求保留以下版权与
署名片段（摘自 RSRD 的 `OGL.txt` §15）：

```text
Open Game License v 1.0a Copyright 2000, Wizards of the Coast, Inc.

System Reference Document Copyright 2000-2003, Wizards of the Coast, Inc.;
Authors Jonathan Tweet, Monte Cook, Skip Williams, Rich Baker, Andy Collins,
David Noonan, Rich Redman, Bruce R. Cordell, John D. Rateliff, Thomas Reid,
James Wyatt, based on original material by E. Gary Gygax and Dave Arneson.

PCGen dataset conversion for the System Reference Document
Copyright 2001-2003, PCgen Data team (Including, but not limited to
Bryan McRoberts, Rob Reed (Mynex), Eric Beaudoin, Andrew McDougall
(Tir Gwaith), Ian Dale, Paul W. King, Chris Chandler (Barak),
Sigurdur H. Olafsson)
```

分发数据包时必须一并保留 `OGL.txt` 与本 NOTICE。完整 OGL 文本随数据快照提供：
`raw/pcgen-master/data/35e/wizards_of_the_coast/rsrd/OGL.txt`。

`data/packs/custom/curated/` 下的本地资料包来源为
`pcgen-BahamutDragon-master`，各包 `pack.json` 中声明为
`OGL-1.0a / PCGen data terms`；这些包**不提交到仓库**（见 `.gitignore`）。

## 3. PF 变体扩展 — 独立重写

`extensions/pf35e/` 是**本项目独立重写**的 Pathfinder 职业变体，
**不复制**官方 PCGen Pathfinder 数据集文件。
Pathfinder 内容属 Paizo Inc.，依 OGL / Community Use Policy 使用。
分发时需保留扩展 `.pcc` 的 `INFOTEXT` 声明与
[`docs/PF35E_EXTENSION.md`](docs/PF35E_EXTENSION.md) 中的范围与有损项说明。

---

## 商标

`Dungeons & Dragons`、`d20 System`、`D&D` 是 Wizards of the Coast, Inc.
的商标；`Pathfinder` 是 Paizo Inc. 的商标。本项目与上述公司无隶属关系，
未获其背书。
