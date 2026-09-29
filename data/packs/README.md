# Data Packs

生成的数据包不提交到仓库；用脚本从固定 PCGen 数据快照打包：

```bash
python3 scripts/package_datapack.py --pack rsrd35 --out data/packs/rsrd35 --zip
python3 scripts/package_datapack.py --pack srd30  --out data/packs/srd30  --zip
```

每个 pack 包含：

```text
pack.json
data/...
```

`pack.json` 记录 `id` / `name` / `entry` / `version` / `data_ref` / `license`。
数据许可与分发条款需按 PCGen / OGL 要求处理。
