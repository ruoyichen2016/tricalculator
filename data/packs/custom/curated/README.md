# Curated Data Packs

这里存放**已经由我们审定/校正后的资料包**。

- 每个资料包都会经过导入检查、语法修正、字段校验；
- 可用于 Web 建卡；
- 启动时优先使用这里的资料。

建议结构：

```text
curated/
  my_pack/
    pack.json
    data/
      ...
```

启动方式：

```bash
PCGEN_DATA_ROOT="$PWD/data/packs/custom/curated/my_pack" ./run-web.sh
```

## 当前已审定资料

- `bahamut35e-core-ua/`：BahamutDragon 3.5e Core Books + Unearthed Arcana；
  已通过 `pcgen import-report` 导入检查，并补全 `_universal/races.lst` 依赖。
