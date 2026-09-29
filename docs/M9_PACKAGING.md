# M9 打包与朋友试用

## 1. 构建发布包

在 `pcgen-rust` 目录：

```bash
python3 scripts/package_release.py --packs rsrd35,srd30 --zip
```

它会：

1. 构建 `pcgen-web` release 二进制；
2. 复制 `run-web.sh` / `run-web.bat`；
3. 调用 `package_datapack.py` 生成：
   - `data/packs/rsrd35/`
   - `data/packs/srd30/`
4. 创建 `characters/`；
5. 可选打成 zip。

产物目录：`dist/pcgen-rust-web/`。

## 2. 朋友使用方式

Windows：

1. 解压发布包；
2. 双击 `run-web.bat`；
3. 浏览器打开 `http://127.0.0.1:8787`；
4. 在 “Create character” 区域建卡；
5. 角色保存到发布目录下的 `characters/`；
6. “Print sheet” 可打印 HTML 角色卡。

Linux / macOS：

```bash
./run-web.sh
```

## 3. 数据包

发布包内置：

- 3.5e RSRD 玩家数据（`rsrd35`）；
- 3.0 SRD 玩家数据（`srd30`）。

默认启动使用 `rsrd35`；要切换 3.0，可设置：

```bash
PCGEN_DATA_ROOT=.../data/packs/srd30 ./run-web.sh
```

## 4. 存档与导出

- 原生角色保存在 `characters/*.json`；
- Web 页面可导入当前 `.pcg` 为原生角色；
- 可导出 `.pcg` 供 PCGen 使用；
- 角色卡可浏览器打印为 PDF。

## 5. 已知限制（beta）

- 不支持 Pathfinder / Modern；
- 不支持随机 KIT；
- 装备购买与官方 output sheet 兼容仍在收敛；
- 首次加载 `.pcc` 需要数秒编译规则包；
- 数据包分发需遵守 PCGen / OGL 许可。

## 6. 故障排查

- 端口 8787 被占用：设置 `PCGEN_WEB_ADDR=127.0.0.1:8788`；
- 启动后浏览器没自动打开：手动访问 `http://127.0.0.1:8787`；
- 找不到数据包：设置 `PCGEN_DATA_ROOT`；
- 找不到二进制：先运行 `cargo build --release -p pcgen-web`。
