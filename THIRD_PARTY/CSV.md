# Minimal native CSV fixture

- Game/source: Stellaris Cygnus 4.5.2, Paradox Interactive.
- Original relative file: `common/component_templates/weapon_components.csv`.
- Original file SHA256: `a0d4b199ea51e59a600beab99046fad3c8a11a49ae39c8ace0aed73ebf6ff330`.
- Excerpt: header at original line 6 and the `SMALL_RED_LASER` row at original line 8.
- Repository fixture: `upstream/tiger-stellar/tests/fixtures/vanilla-red-laser.csv`.
- Fixture size: 452 bytes, containing the header and one row.
- Fixture SHA256: `fabdc0fd3345bff16b1d4a886e5fd447af665c3d7bfa88ea92033339422cfb28`.

The excerpt is used only for minimal CSV parsing, reference, damage/timing and override regression controls. It does not represent the entire native component definition, gameplay balance or engine behavior. The complete source CSV, game code and game assets are not distributed with this fixture. Preserve its original bytes; `.gitattributes` marks CSV files as `-text`.
