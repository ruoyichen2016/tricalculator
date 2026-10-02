# Custom Data Packs

把自定义资料包放到这个目录下，每个资料包一个子目录：

```text
data/packs/custom/
  my_pack/
    pack.json
    data/
      ...
```

`pack.json` 示例：

```json
{
  "id": "my_pack",
  "name": "My Custom Pack",
  "entry": "data/custom/main.pcc",
  "version": "1.0.0",
  "data_ref": "local",
  "license": "OGL-1.0a"
}
```

启动 Web 时可用 `PCGEN_DATA_ROOT` 指向该资料包：

```bash
PCGEN_DATA_ROOT="$PWD/data/packs/custom/my_pack" ./run-web.sh
```

也可以把多个资料包都放在 `custom/` 下，由后续的数据包选择器统一发现。
当前 `custom/` 只保留手动放置的资料包，不做自动打包。
