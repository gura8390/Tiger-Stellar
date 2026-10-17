# Changelog

## 0.1.8 — 2026-10-17

- Added implicit AND and scripted type-guard narrowing, retaining uncertainty for unsupported or recursive guards.
- Checked ordinary and weighted on_action event lists and propagated preset FROM context; missing context in one caller remains unknown rather than borrowing another caller's type.
- Extended event-picture resource references, special-project condition descriptions, and agreement-term value checks.
- Retained the existing JSON output, severity and diagnostic keys. Added targeted positive and negative regression tests.

中文：补隐式 AND 与脚本类型守卫、on_action 事件列表和预设 FROM 传播、事件图片资源、特殊项目条件描述及协议条款值检查；保留未知上下文和现有诊断格式。

## 0.1.7 — 2026-10-17

- Refined warning reasons and scope evidence in the existing JSON output while retaining the original severity and diagnostic keys.
- Extended fleet action queues, situation creation, arrival callbacks, parameterized inline scripts, and selected reference contracts.
- Added targeted regression coverage for the new checks. No new human-readable reporting interface is introduced.

中文：细化现有 JSON 的告警原因和作用域证据，补充舰队行动队列、局势创建、到达回调、参数内联及部分引用检查。保留原有严重级别和诊断键。

## Included changes from 0.1.5–0.1.6

- Fixed a crash involving inline weapon definitions during vanilla scans, and improved structural checks, relative asset paths, parameter grammar, and scope contracts.
- Added component-condition reasoning, vanilla call-entry inference, and additional parameter substructures.

中文：修复原版扫描中的内联武器崩溃，补结构、资源相对路径和参数子语法检查；增加组件条件证明与调用入口推导。

## Validation limits

Published source is checked with the standalone parser workspace and synthetic regression tests. Vanilla scans can complete, but retained errors and coverage warnings remain; completion does not mean full semantic coverage or zero false positives. Static checks do not establish engine, gameplay, or save-game acceptance.

0.1.8 validation is targeted; a new full vanilla scan is not claimed for this release.
