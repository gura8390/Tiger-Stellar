# Tiger Stellar 0.1.9tmp

Windows static checker for **Stellaris Cygnus 4.5.2** mods. The checker and its isolated syntax worker run in Rust; there is no Python semantic runtime or Python fallback.

群星模组静态检查器。此版本为 Rust 原生实现的实验临时版，目标游戏版本为 Stellaris 4.5.2。

The displayed version and Git tag are `0.1.9tmp`. Cargo uses the valid SemVer package version `0.1.9-tmp`; these names identify the same temporary version.

## Build and test

Requirements: Windows, PowerShell, and an installed Rust toolchain with Cargo on `PATH`. The selected toolchain must have its required linker and Windows build dependencies available. The build does not install a Rust toolchain or download game files.

```powershell
.\build.ps1
```

The script builds the locked Rust workspace, runs its release-mode workspace tests against the newly built checker, and writes `build/bin/tiger-stellar.exe` plus its SHA256 file. Tests use small local fixtures and do not require an installed game.

Temporary files, Cargo's package cache and the target directory default to directories under `build/`. Explicit `CARGO_HOME` and `CARGO_TARGET_DIR` values, or the script's `-CargoHome` and `-TargetDir` parameters, select alternate locations. The script restores its process environment when it finishes. Integration-test fixture trees are generated under the repository's ignored `verification/` directory.

## Usage

```powershell
.\build\bin\tiger-stellar.exe --game "<Stellaris directory>" --mod "<mod directory>" --output .\build\reports\mod.json --language simp_chinese
```

`--game` and `--output` are required. The game directory must contain Stellaris's `launcher-settings.json`. The mod argument is the actual mod directory.

| Option | Purpose |
| --- | --- |
| `--mod PATH` | Select the final mod being checked. Omit it to check the supplied game inputs. |
| `--dependency PATH` | Add a dependency; repeat in load order before the final mod. |
| `--language LANGUAGE` | Check localization against a selected language, for example `simp_chinese`. |
| `--cache PATH` | Select an optional parser-result cache directory. |
| `--rules FILE` | Use a supplied Stellaris rules JSON. |
| `--syntax-only` | Check syntax without claiming semantic validation. |
| `--version`, `--help` | Print the version or supported command line. |

Game-version differences produce a warning and validation continues. Inputs for another game are rejected. File overrides and `replace_path` follow the supplied dependency order. The JSON report retains diagnostic severity, source locations, available call-chain evidence and input hashes.

| Exit code | Meaning |
| --- | --- |
| `0` | No errors or warnings, with a nonempty checked target. |
| `1` | At least one reported error. |
| `2` | No errors, but warnings or an empty target. This is not a clean validation result. |
| `3` | Input, argument or other infrastructure failure. |

## Validation boundary

**Experimental, partial semantic coverage.** Full DSL coverage, complete parity with 0.1.8, and game-engine acceptance have not been established. Unknown contracts remain visible as warnings. A static result does not prove gameplay, save compatibility, rendering quality, runtime target availability or performance in the game.

Supported checks include modeled trigger/effect scopes, script and macro calls, references, selected declaration contracts, numeric expressions and resource files. Guard and saved-target type evidence do not establish that an engine callback actually runs or that a target exists at runtime. Some diagnostics can originate in inherited game code; retain and investigate them with their original severity.

The syntax worker may reuse a bounded, successful reader-header compilation. It still parses each body with a fresh local overlay; this cache does not store semantic states or suppress failed-header diagnostics.

中文：未实现或无法证明的合同继续报告为未知。零错误不能替代引擎验收，也不代表完整覆盖。请结合原版接口、正反例和实际游戏测试判断结果。

## Credits and license

The syntax core is derived from [amtep/tiger](https://github.com/amtep/tiger), originally based on upstream commit [`9e3d3b25`](https://github.com/amtep/tiger/commit/9e3d3b25c7524cdcc86d5f4190f3f8cffbc4d630), with local changes. Thanks to @amtep and the [upstream contributors](CONTRIBUTORS.md). Tiger Stellar is independently maintained and is not an official upstream release. Its standalone workspace does not link other Tiger game backends.

Project code is licensed under **GPL-3.0-or-later**; see [LICENSE](LICENSE). Original notices and `CONTRIBUTORS.md` are retained. See [third-party notes](THIRD_PARTY/README.md) for the minimal game-data test excerpt.
