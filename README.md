# Tiger Stellar 0.1.7

A Windows static checker for **Stellaris Cygnus 4.5.2 (9776)** mods, with a Tiger-derived Rust syntax parser and a separate Python semantic checker.

群星模组静态检查器：基于 Tiger 解析器，独立实现群星规则、作用域检查、实体引用与多模组覆盖分析。

## Latest changes / 最新变化

**0.1.7** adds more precise warning reasons and scope evidence in JSON, plus fleet action queues, situation creation, arrival callbacks, parameterized inline scripts, and selected reference checks. It also includes the crash, asset-path, and semantic coverage fixes from 0.1.5–0.1.6. See [CHANGELOG.md](CHANGELOG.md) for details and validation limits.

## Credits and thanks / 致谢

The original [Tiger](https://github.com/amtep/tiger) was created by [@amtep](https://github.com/amtep). Thank you to @amtep and the [upstream contributors](CONTRIBUTORS.md) for the time, expertise, and care invested in Tiger. This project adapts its syntax core and adds a separate Stellaris semantic layer; it is independently maintained and is not an official upstream release.

感谢原作者 [@amtep](https://github.com/amtep) 及所有上游贡献者创建和维护 Tiger。本项目复用并适配其语法解析核心，新增独立的群星语义检查层，感谢他们的付出与开源贡献。

The syntax core is derived from upstream commit [`9e3d3b25`](https://github.com/amtep/tiger/commit/9e3d3b25c7524cdcc86d5f4190f3f8cffbc4d630), with subsequent local changes. The standalone parser does not link Tiger's other game backends. Original notices are retained. This repository is licensed under **GPL-3.0-or-later**; see [LICENSE](LICENSE).

## Build and test

Requirements: Windows, Python 3.12 or later, and a current stable Rust toolchain. Rust must be on `PATH`, with the build tools required by the selected Windows toolchain (Visual Studio C++ Build Tools for MSVC, or a configured GNU toolchain).

```powershell
.\build.ps1
```

This builds `dist/stellaris-parser.exe` and runs the Python regression suite. It uses only the standalone workspace under `upstream/`; no other game backend or local game installation is needed for these tests. To additionally package the Windows application:

```powershell
python -m pip install -r requirements-build.txt
.\build.ps1 -Package
```

## Usage

```powershell
python -X utf8 .\src\stellar.py --game "C:\Games\Stellaris" --mod "C:\Mods\MyMod" --cache .\cache --output .\reports\my-mod.json
```

For a packaged build, use `dist/tiger-stellar/tiger-stellar.exe` with the same arguments. Run `--help` for all options. Repeat `--dependency <directory>` in load order; `--mod` is the final checked mod. The checker respects `replace_path` and file overrides. `--native-control` compares diagnostics against a targeted vanilla baseline. Game files and Workshop dependencies must be supplied locally; they are not included or downloaded automatically.

## Coverage and limits / 检查边界

Checks include syntax, supported trigger/effect scopes, scope links, scripted calls and macros, common entity references, modifiers, ship/component contracts, localization references, and layered mod definitions. Rule extraction tools are included under `tools/` for maintainers with a local game installation and engine-generated script documentation.

**Experimental, partial semantic coverage.** Unknown or incompletely supported constructs remain visible as coverage warnings. Diagnostics can include existing vanilla issues and false positives; they are not proof that a mod is broken or correct. Full semantic validation and in-game/save-game acceptance have not been established. GUI/rendering semantics, some parameter structures, and dynamic contexts remain incomplete. Rule tables target the version above and need review after game updates.

本版本已完成相应交付，但仍是实验性的有限语义检查器。未覆盖区域保留告警；检查结果不等于引擎、玩法或存档验收，也不保证无误报。

This repository contains runnable source, rules, synthetic regression fixtures, and build instructions. Machine-specific handoffs, design documents, private verification copies, game assets, caches, and release archives are excluded.


## Implementation overview / 实现范围

| Component | What this project adds or adapts |
|---|---|
| Syntax parser | Standalone Tiger-derived Rust syntax core; no other game backend is linked. |
| Stellaris rules | Versioned Stellaris 4.5.2 scope/API and declaration tables. |
| Semantic checker | Layered definitions, references, scope checks, scripted calls, component/ship contracts and localization checks in Python. |
| Diagnostics | Targeted vanilla comparisons, provenance, override reports and explicit coverage warnings. |
| Regression suite | 352 source-level tests and 9 Rust parser tests passed for the 0.1.7 publication; no in-game acceptance is implied. |

This repository currently publishes source rather than binary Release downloads. For the separate CK3 adaptation, see [Tiger CK3](https://github.com/gura8390/Tiger-CK3).
