# M9 朋友 Beta Checklist

> 目标：让普通朋友在 Windows / Linux 上解压即用，建 3.0 / 3.5e 角色，
> 保存、打印、导出 `.pcg`，并能把问题带 diagnostics 反馈回来。

## 1. 生成发布包

```bash
python3 scripts/package_release.py --packs rsrd35,srd30 --zip
```

产物：`dist/pcgen-rust-web/`（以及 `.zip`）。

## 2. 朋友安装

> **平台注意**：Windows 必须使用 Windows 上构建的发布包（内含
> `pcgen-web.exe`）。如果拿到的是 Linux 构建包，`run-web.bat` 会因缺少
> `pcgen-web.exe` 而报错/闪退；请重新在 Windows 上打包，或获取 Windows 构建。

Windows：

1. 解压 zip；
2. 双击 `run-web.bat`；
3. 浏览器打开 `http://127.0.0.1:8787`。

Linux / macOS：

```bash
./run-web.sh
```

## 3. 必测场景

### A. 建卡

- [ ] 3.5e Fighter，1 级，购点 25；
- [ ] 3.5e Wizard，3 级，选择法术；
- [ ] 3.5e Cleric，5 级，选择神祇 / 领域；
- [ ] 3.5e Rogue，10 级，技能点分配；
- [ ] 3.0 角色：切换 `PCGEN_DATA_ROOT` 到 `data/packs/srd30` 后重复 1 个 case。

### B. 数值

- [ ] 购点超支时页面报错；
- [ ] 技能点 / 专长槽不足时有提示；
- [ ] HP、BAB、AC、豁免与预期一致；
- [ ] 升级 1 级后 HP / BAB / 技能正确更新。

### C. 角色管理

- [ ] 保存角色；
- [ ] 刷新页面后仍能打开；
- [ ] 复制 / 删除角色；
- [ ] 同时存在多个角色时不串档。

### D. 输出

- [ ] Print sheet 能打印 / 导出 PDF；
- [ ] Export `.pcg` 能在 PCGen 中打开；
- [ ] 角色列表能显示名字 / 等级 / 种族。

### E. 稳定性

- [ ] 连续建卡 5 个不退崩溃；
- [ ] 首次加载等待可接受（< 10s）；
- [ ] 第二次操作响应 < 1s。

## 4. 反馈模板

请把下面内容一起发给开发者：

```text
系统：Windows 10 / 11 / Linux
浏览器：Chrome / Edge / Firefox
操作步骤：
预期结果：
实际结果：
能否重现：
diagnostics（点 Copy diagnostics 按钮后粘贴）：
```

## 5. 已知限制（Beta）

- 不支持 Pathfinder / Modern；
- 不支持随机 KIT；
- 装备购买与官方 output sheet 兼容仍在收敛；
- 首次加载 `.pcc` 需要数秒编译；
- 数据包分发需遵守 PCGen / OGL 许可。
