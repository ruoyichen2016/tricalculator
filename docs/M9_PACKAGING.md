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

> **平台注意**：发布包必须与操作系统匹配。当前仓库内 `dist/` 下的
> `pcgen-rust-web-0.1.0.zip` 是在 Linux 环境构建的，仅含 Linux 二进制；
> Windows 用户需要拿到在 Windows 上执行 `scripts/package_release.py` 生成的
> 发布包（内含 `pcgen-web.exe`）。否则 `run-web.bat` 会提示找不到
> `pcgen-web.exe`。

Windows：

1. 解压 Windows 版发布包；
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
- 数据包分发需遵守 OGL 1.0a / PCGen 数据条款：`package_datapack.py` 会在包根
  写入 `NOTICE.md`（含必须保留的 OGL §15 署名与商标声明），OGL 全文随数据树
  一同复制（rsrd35 为 `data/35e/wizards_of_the_coast/rsrd/OGL.txt`，
  srd30 为 `data/3e/wizards_of_the_coast/srd/OGL.txt`）。
  仓库自身的许可划分见 [`NOTICE.md`](../NOTICE.md)。

## 6. 故障排查

- 端口 8787 被占用：设置 `PCGEN_WEB_ADDR=127.0.0.1:8788`；
- 启动后浏览器没自动打开：手动访问 `http://127.0.0.1:8787`；
- 找不到数据包：设置 `PCGEN_DATA_ROOT`；
- 找不到二进制：先运行 `cargo build --release -p pcgen-web`。
