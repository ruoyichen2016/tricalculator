# Cohort Matrix

> 由 `scripts/cohort_matrix.py --write` 生成；`--check` 在 full-data CI 中保证每个 fixture 都被归类。

## Summary

- PCG files: **42**
- XML csheets: **32**
- HTM csheets: **18**
- Primary cohort: **25**
- Extended cohort: **2**
- Non-primary: companion=2, legacy=3, out-of-scope=7, unit-test=3

## Primary cohort

| PCG | Status | Sheets | Reason |
|---|---|---|---|
| `35e_Alice` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Bard` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Bob` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Charlie` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_CloudGiantHalfDragon` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Dave` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Eve` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Fran` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Gordon` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Harold` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Ivan` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Jango` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_JimDop` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_L10(fighter-human)` | included-primary | htm(35e_L10(fighter-human-cmb).htm, 35e_L10(fighter-human-cmp).htm, 35e_L10(fighter-human-std).htm, 35e_L10(fighter-human_sb2).htm) | 主验收 cohort（3.0/3.5e） |
| `35e_L7-3(fighter-cleric-dwarf)` | included-primary | htm(35e_L7-3(fighter-cleric-dwarf-cmb).htm, 35e_L7-3(fighter-cleric-dwarf-cmp).htm, 35e_L7-3(fighter-cleric-dwarf-sb2).htm, 35e_L7-3(fighter-cleric-dwarf-std).htm) | 主验收 cohort（3.0/3.5e） |
| `35e_Lauren` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Mallory` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `35e_Quasvin` | included-primary | xml | 主验收 cohort（3.0/3.5e） |
| `3e_BarJack` | included-primary | xml htm(3e_BarJack.htm) | 主验收 cohort（3.0/3.5e） |
| `3e_BrdJoe` | included-primary | xml htm(3e_BrdJoe.htm) | 主验收 cohort（3.0/3.5e） |
| `3e_CleElf` | included-primary | xml htm(3e_CleElf.htm) | 主验收 cohort（3.0/3.5e） |
| `3e_FigFae` | included-primary | xml htm(3e_FigFae.htm) | 主验收 cohort（3.0/3.5e） |
| `3e_MonKee` | included-primary | xml htm(3e_MonKee.htm) | 主验收 cohort（3.0/3.5e） |
| `3e_SWizSam` | included-primary | xml htm(3e_SWizSam.htm) | 主验收 cohort（3.0/3.5e） |
| `3e_WizShar` | included-primary | xml htm(3e_WizShar.htm) | 主验收 cohort（3.0/3.5e） |

## Extended cohort

| PCG | Status | Sheets | Reason |
|---|---|---|---|
| `msrd_Elwood` | included-extended | xml | MSRD extended visibility，不参与主验收 |
| `msrd_Ilyana` | included-extended | xml | MSRD extended visibility，不参与主验收 |

## Non-primary fixtures

| PCG | Status | Sheets | Reason / next action |
|---|---|---|---|
| `35e_Bob_Companion` | companion | - | companion / psicrystal，需要独立 companion 入口 |
| `35e_Q-PsiCrystal` | companion | xml | companion / psicrystal，需要独立 companion 入口 |
| `35e_UT~Gordon` | unit-test | - | PCGen 单元测试用 PCG，不是完整角色 |
| `35e_UT~Harold` | unit-test | - | PCGen 单元测试用 PCG，不是完整角色 |
| `35e_UT~Ivan` | unit-test | - | PCGen 单元测试用 PCG，不是完整角色 |
| `Zen_Nancy` | legacy | - | legacy PCG / 无完整 campaign 映射 |
| `none_PdfBuster` | legacy | - | legacy PCG / 无完整 campaign 映射 |
| `none_derth` | legacy | htm(none_derth.htm) | legacy PCG / 无完整 campaign 映射 |
| `pf_Cleric` | out-of-scope | xml | Pathfinder / Starfinder，产品范围外 |
| `pf_Paladin` | out-of-scope | xml | Pathfinder / Starfinder，产品范围外 |
| `pf_Rogue` | out-of-scope | xml | Pathfinder / Starfinder，产品范围外 |
| `pf_goldielocks` | out-of-scope | xml | Pathfinder / Starfinder，产品范围外 |
| `sf_mechanic` | out-of-scope | xml | Pathfinder / Starfinder，产品范围外 |
| `sf_mechanic_drone` | out-of-scope | - | Pathfinder / Starfinder，产品范围外 |
| `sf_soldier` | out-of-scope | xml | Pathfinder / Starfinder，产品范围外 |

## Csheet-only artifacts

- `EntireParty`: 无对应 PCG，保留为测试套件 artifact/长角色表。
- `csheet`: 无对应 PCG，保留为测试套件 artifact/长角色表。

## 未纳入裁决

- `35e_Q-PsiCrystal`：companion / psicrystal。Rust 可计算但 HP/BAB 为 0，需要独立 companion 入口后再纳入，不进入 primary。
- `none_derth`：PCG v2.4.3 legacy 格式，无 CAMPAIGN 行；当前不映射到任何验收 PCC，保留为 legacy 审计样本。
- `pf_*` / `sf_*`：Pathfinder / Starfinder，明确在 PRODUCT_SCOPE 之外。

