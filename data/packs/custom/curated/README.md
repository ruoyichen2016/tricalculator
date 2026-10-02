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
