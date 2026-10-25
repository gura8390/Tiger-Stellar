# Changelog

## 0.1.9tmp — experimental Rust version

Display version and Git tag: `0.1.9tmp`. Cargo package version: `0.1.9-tmp`.

- Replace the Python semantic runtime with a native Rust checker and an isolated Rust syntax worker.
- Strengthen modeled scope guards, script/macro expansion, saved-target type evidence, numeric contracts and selected declaration/callback checks. Preserve unknown contracts instead of treating them as proven valid.
- Check synthetic macro-wrapper boundaries. Escaped roots retain static diagnostics and do not supply certain type, target or numeric facts.
- Reuse successful reader-header compilation in a bounded worker-local cache. Each body keeps a fresh local overlay; failed headers and their diagnostics are not cached as successful input.
- Preserve game-version mismatch warnings and continue checking. Empty targets do not count as clean validation.
- Provide a locked Rust build and workspace tests without Python packaging.

This is a temporary experimental version. Complete DSL coverage, full 0.1.8 parity and engine/runtime acceptance remain unproven. Retained warnings are part of the validation boundary. No gameplay or game-speed claim is made.

## 0.1.8

The preceding published implementation used a Python semantic checker with a Tiger-derived Rust syntax parser. Its historical source and publication remain available in repository history. That runtime is not required by 0.1.9tmp.
