# Custom Data Packs

该目录区分两类资料：

| 目录 | 用途 |
|---|---|
| `original/` | 原始资料包，未经审定，可能无法直接导入 |
| `curated/` | 已经审定/校正后的资料包，可用于 Web 建卡 |

工作流程：

1. 把从别处拿到的资料包放入 `original/`；
2. 开发者对其进行导入检查与修正；
3. 修正后可用的资料放入 `curated/`；
4. Web 启动时通过 `PCGEN_DATA_ROOT` 指向 `curated/` 下的具体资料包。
