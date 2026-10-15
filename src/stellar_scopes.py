"""Scope vocabulary pinned to the local Stellaris 4.5.2 engine API export.

The vocabulary is deliberately independent of Tiger's other game scope enums.
Update it only after inspecting a new Stellaris engine export and its provenance.
"""
ENGINE_SCOPE_TYPES=frozenset('''agreement all ambient_object archaeological_site army
astral_rift bypass colony cosmic_storm cosmic_storm_influence_field country debris
deposit design dlc_recommendation espionage_asset espionage_operation exhibit federation
first_contact fleet galactic_community galactic_object job leader megastructure mission
no_scope planet pop pop_faction pop_group sector ship ship_growth_stage situation
species spy_network starbase trait various war'''.split())
