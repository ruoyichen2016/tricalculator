# M9 Loop 执行状态

> 目标：把 pcgen-rust 推进到“自用 + 普通朋友可建卡”的 Web 产品。
> 每轮执行一个 Item：实现 → 门禁 → commit → 更新本文件。

## Items

- [x] **M9-0 范围冻结**：购点 / 3.0+3.5e / 打包数据 / Web 优先 /
      无随机 KIT；见 `docs/M9_PERSONAL_BUILDER.md`。
- [~] **M9-1 原生存档 + 角色仓库 + Web 角色管理**
  - [x] `pcgen-builder` crate：`NativeSaveEnvelope` + `CharacterRepository`
  - [ ] Web `/api/characters` 列表 / 新建 / 打开 / 删除
  - [ ] Web 角色列表页面
- [ ] **M9-2 购点 + create / level-up**
  - [ ] 3.5e 购点表 + 3.0 兼容
  - [ ] builder create / level-up workflow + validation
  - [ ] CLI create / level-up
- [ ] **M9-3 Web 创建向导 + 角色卡打印**
- [ ] **M9-4 数据包打包 + 启动器 + 朋友 beta**

## 当前状态

- 最近更新：M9-1a 完成（builder crate + 5 tests），M9-1b Web 角色管理待做。
- 已知风险：数据包许可、购点表版本、Web 首次编译性能。
