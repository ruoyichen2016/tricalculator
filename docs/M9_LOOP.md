# M9 Loop 执行状态

> 目标：把 pcgen-rust 推进到“自用 + 普通朋友可建卡”的 Web 产品。
> 每轮执行一个 Item：实现 → 门禁 → commit → 更新本文件。

## Items

- [x] **M9-0 范围冻结**：购点 / 3.0+3.5e / 打包数据 / Web 优先 /
      无随机 KIT；见 `docs/M9_PERSONAL_BUILDER.md`。
- [x] **M9-1 原生存档 + 角色仓库 + Web 角色管理**
  - [x] `pcgen-builder` crate：`NativeSaveEnvelope` + `CharacterRepository`
        （commit `b21614a`）
  - [x] Web `/api/characters` 列表 / 新建 / 打开 / 删除
        （commit `a2fbc10`）
  - [x] Web 角色列表与管理页面
- [x] **M9-2 购点 + create / level-up**
  - [x] 3.5e 购点表 + 3.0 兼容（共用表，预算可配）
  - [x] builder create / level-up workflow + validation
  - [x] CLI create / level-up（commit `b3fcd5d`）
- [x] **M9-3 Web 创建向导 + 角色卡打印**（commit `f679d44`）
- [x] **M9-4 数据包打包 + 启动器**（commit `342f40b`）
  - [x] `package_datapack.py` / `package_release.py`
  - [x] `run-web.sh` / `run-web.bat`
  - [x] `docs/M9_PACKAGING.md`
- [~] **M9-5 朋友 beta 反馈与修复**
  - [x] M9-5a Beta checklist + package dry-run + Copy diagnostics
        （commit `de9de22`）
  - [ ] M9-5b 真实 beta 反馈修复
- [~] **M9-6 建卡深化**
  - [x] M9-6a 装备购买 + 起始金币（commit `32f95dc`）
  - [x] M9-6b 法术书编辑
  - [ ] M9-6c output sheet 深化

## 当前状态

- 最近更新：M9-6a 完成（commit `32f95dc`）；下一轮 M9-6b 法术书编辑。
- 已知风险：数据包许可、购点表版本、Web 首次编译性能。
