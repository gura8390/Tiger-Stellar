"""Vanilla audit regressions: contracts, overlays and rejected mutations."""
import json,tempfile,unittest
from pathlib import Path
from test_coverage_011 import ast,checker,ROOT
from stellar import Context,load,fields,children

class Vanilla015(unittest.TestCase):
 def clean(self,c):self.assertEqual([],c.diagnostics)
 def error(self,c,key):self.assertTrue(any(d['key']==key and d['severity']=='error' for d in c.diagnostics),c.diagnostics)
 def component(self,tail):
  c=checker();c.entities['technology'].add('known');c.entities['economic_categories'].add('ships');c.entities['strategic_resources'].add('energy')
  c.definition('component_templates',ast('weapon_component_template={key=TEST damage={min=1 max=2} total_fire_time=30 '+tail+'}')[0]);return c
 def test_valid_structure(self):self.clean(self.component('prerequisites={known} resources={category=ships cost={energy=10}} ship_modifier={ship_shield_mult=1}'))
 def test_malformed_prerequisites(self):self.error(self.component('prerequisites={unexpected=absent}'),'structure')
 def test_scalar_prerequisites(self):self.error(self.component('prerequisites=known'),'structure')
 def test_nested_prerequisites(self):self.error(self.component('prerequisites={{known}}'),'structure')
 def test_missing_prerequisite(self):self.error(self.component('prerequisites={absent}'),'missing-item')
 def test_logical_prerequisites(self):self.clean(self.component('prerequisites={OR={known AND={known}}}'))
 def test_logical_missing_prerequisite(self):self.error(self.component('prerequisites={OR={missing known}}'),'missing-item')
 def test_scalar_resources(self):self.error(self.component('resources=123'),'structure')
 def test_scalar_cost(self):self.error(self.component('resources={cost=123}'),'structure')
 def test_scalar_upkeep(self):self.error(self.component('resources={upkeep=123}'),'structure')
 def test_scalar_cost_trigger(self):self.error(self.component('resources={cost={trigger=yes}}'),'structure')
 def test_scalar_modifier(self):self.error(self.component('ship_modifier=123'),'structure')
 def test_scalar_triggered_modifier(self):self.error(self.component('triggered_ship_modifier=123'),'structure')
 def test_missing_component_key(self):
  c=checker();c.definition('component_templates',ast('weapon_component_template={damage={min=1 max=2} total_fire_time=30}')[0]);self.error(c,'missing-field')
 def test_nested_inline_index_and_validation_share_tree(self):
  with tempfile.TemporaryDirectory() as t:
   game=Path(t);p=game/'common/component_templates/t.txt';p.parent.mkdir(parents=True);p.write_text('weapon_component_template={inline_script={script=outer TIER=1} prerequisites={known}}')
   q=game/'common/inline_scripts/outer.txt';q.parent.mkdir(parents=True);q.write_text('key=GUN_$TIER$ damage={min=1 max=2} total_fire_time=30 resources={inline_script=cost}')
   r=q.with_name('cost.txt');r.write_text('cost={energy=10}')
   c=checker();c.entities['technology'].add('known');c.entities['strategic_resources'].add('energy');target=load(c,game,[])
   node=next(ns[0] for _,category,ns in target if category=='component_templates');self.assertEqual('GUN_1',fields(node)['key']['value']);self.assertEqual('GUN_1',fields(c.definitions['component_templates']['GUN_1'])['key']['value']);self.assertNotIn('inline_script',fields(fields(node)['resources']))
   self.assertEqual(str(q),fields(node)['key']['_source_path']);c.definition('component_templates',node);self.clean(c)
   r.write_text('cost=123');c=checker();c.entities['technology'].add('known');target=load(c,game,[]);node=next(ns[0] for _,category,ns in target if category=='component_templates');c.definition('component_templates',node);self.error(c,'structure')
   self.assertEqual(str(r),next(d['path'] for d in c.diagnostics if d['key']=='structure'))
 def test_recursive_inline_terminates_with_gap(self):
  with tempfile.TemporaryDirectory() as t:
   c=checker();p=Path(t)/'loop.txt';p.write_text('inline_script=loop');c.inline={'loop':p};c.file=str(p)
   c.expanded_structure(ast('x={inline_script=loop}')[0],Context('ship','ship'));self.assertTrue(any(d['key']=='validation-coverage' for d in c.diagnostics));self.assertEqual([],c.active)
 def test_decimal_variable_values_are_not_scope_paths(self):
  c=checker();c.script(ast('multiply_variable={which=x value=0.5} subtract_variable={which=y value=-1.25}'),'effects',Context('fleet','fleet'));self.clean(c);self.assertEqual('number',c.variable_types['fleet','x'])
 def test_switch_keeps_context_and_checks_children(self):
  c=checker();c.script(ast('switch={trigger=is_difficulty 0={multiply_variable={which=x value=0.5}} default={multiply_variable={which=x value=1.5}}}'),'effects',Context('fleet','fleet'));self.clean(c)
  c.script(ast('switch={trigger=is_difficulty 0={set_planet_flag=bad}}'),'effects',Context('fleet','fleet'));self.error(c,'scope')
 def test_unknown_switch_selector(self):
  c=checker();c.script(ast('switch={trigger=typo 0={}}'),'effects',Context('fleet','fleet'));self.error(c,'unknown-trigger')
 def aura(self,body):
  c=checker();c.entities['gfx_entity'].add('circle');c.definition('component_templates',ast('utility_component_template={key=A friendly_aura={name=aura stack_info={id=stack} '+body+'}}')[0]);return c
 def test_aura_graphics_and_optional_priority(self):self.clean(self.aura('graphics={area_effect={entity=circle dynamic_scale=yes} ship_effect={entity=circle dynamic_scale=no}}'))
 def test_aura_graphics_entity(self):self.error(self.aura('graphics={area_effect={entity=missing}}'),'missing-item')
 def test_aura_graphics_boolean(self):self.error(self.aura('graphics={area_effect={entity=circle dynamic_scale=maybe}}'),'value')
 def test_aura_graphics_typo(self):self.error(self.aura('graphics={area_effect={entity=circle dynamik_scale=yes}}'),'unknown-field')
 def test_aura_graphics_scalar(self):self.error(self.aura('graphics=123'),'structure')
 def test_aura_graphics_missing_entity(self):self.error(self.aura('graphics={area_effect={dynamic_scale=yes}}'),'missing-field')
 def test_aura_priority_number(self):self.error(self.aura('stack_info={id=stack priority="!"}'),'value')
 def names(self,body):
  c=checker();c.loc.add('NAME');c.name_list(ast('bio={'+body+'}')[0]);return c
 def test_biogenesis_names(self):self.clean(self.names('selectable={always=no} randomized=no by_rarity_names={common={NAME} rare={NAME} epic={NAME} exceptional={NAME}} design_names={NAME}'))
 def test_name_rarity_typo(self):self.error(self.names('by_rarity_names={commmon={NAME}}'),'unknown-field')
 def test_name_rarity_structure(self):self.error(self.names('by_rarity_names={common={bad=NAME}}'),'name-list-structure')
 def test_name_selectable_scope(self):self.error(self.names('selectable={is_star=yes}'),'scope')
 def test_name_selectable_is_scope_free(self):self.error(self.names('selectable={is_ai=no}'),'scope')
 def test_name_country_trigger(self):self.clean(self.names('trigger={is_ai=no} should_name_home_system_planets=no'))
 def test_name_boolean_rejected(self):self.error(self.names('should_name_home_system_planets=maybe'),'value')
 def test_name_selectable_shape(self):self.error(self.names('selectable=yes'),'structure')
 def test_event_option_metadata(self):
  c=checker();c.event(ast('country_event={id=t.1 option={hide_option_if_not_allowed=yes is_dialog_only=no default_hide_option=yes}}')[0]);self.clean(c)
  c.event(ast('country_event={id=t.2 option={hide_option_if_not_allowed=maybe}}')[0]);self.error(c,'value')
 def test_event_unknown_option_not_ignored(self):
  c=checker();c.event(ast('country_event={id=t.1 option={hide_optiom_if_not_allowed=yes}}')[0]);self.error(c,'unknown-effect')
 def test_event_option_icon_structure(self):
  c=checker();c.loc.add('TIP');c.event(ast('country_event={id=t.1 option={icon={icon=GFX_test text=TIP}}}')[0]);self.clean(c)
  c.event(ast('country_event={id=t.2 option={icon={wrong=GFX_test}}}')[0]);self.error(c,'unknown-field')
 def test_nested_conditional_picture_inline(self):
  with tempfile.TemporaryDirectory() as t:
   p=Path(t)/'picture.txt';p.write_text('picture={trigger={is_ai=no} picture=GFX_example}');c=checker();c.inline={'picture':p};c.file=str(p)
   c.event(ast('country_event={id=t.1 picture={trigger={always=yes} inline_script=picture}}')[0]);self.clean(c)
   p.write_text('picture={trigger={is_star=yes} picture=GFX_example}');c.event(ast('country_event={id=t.2 picture={inline_script=picture}}')[0]);self.error(c,'scope')
 def test_damage_fixed_and_area_ranges(self):
  self.clean(self.component('damage=12 collateral_damage=20 chain_damage={min=1 max=9}'))
  self.error(self.component('chain_damage={min=10 max=2}'),'range-order');self.error(self.component('collateral_damage=-1'),'value')
 def test_component_size_enum(self):
  self.clean(self.component('size=LARGE'));self.error(self.component('size=audited_size_typo'),'value')
 def test_component_design_scope_contract(self):
  self.clean(self.component('potential={is_ship_size=starbase_citadel from={is_ai=no}}'))
  self.error(self.component('potential={set_ship_flag=wrong}'),'unknown-trigger')
 def test_resource_scalar_reference(self):
  c=checker();c.entities['strategic_resources'].add('energy');c.script(ast('has_resource=energy has_resource=no'),'triggers',Context('planet','planet'));self.clean(c)
  c.script(ast('has_resource=missing'),'triggers',Context('planet','planet'));self.error(c,'missing-item')
 def test_new_creation_parameters(self):
  c=checker();c.script(ast('create_fleet={parent=this set_take_point=yes}'),'effects',Context('fleet','fleet'));self.clean(c)
  c.script(ast('create_fleet={parent=this set_take_point=maybe}'),'effects',Context('country','country'));self.error(c,'scope');self.error(c,'value')
 def test_parameterized_decimal_value_is_not_a_scope(self):
  c=checker();c.entities['script_values'].add('number_value');c.script(ast('set_variable={which=x value=value:number_value|MULT|0.5|}'),'effects',Context('planet','planet'))
  self.assertTrue(c.diagnostics);self.assertTrue(all(d['key']=='validation-coverage' for d in c.diagnostics),c.diagnostics)
 def test_scoped_parameterized_decimal_value(self):
  c=checker();c.entities['script_values'].add('number_value');c.script(ast('set_variable={which=x value=root.value:number_value|MULT|0.5|}'),'effects',Context('planet','country'))
  self.assertTrue(c.diagnostics);self.assertTrue(all(d['key']=='validation-coverage' for d in c.diagnostics),c.diagnostics)
 def test_scoped_value_bad_prefix_still_errors(self):
  c=checker();c.entities['script_values'].add('number_value');c.script(ast('set_variable={which=x value=wrong_scope.value:number_value|MULT|0.5|}'),'effects',Context('planet','country'));self.error(c,'unknown-scope')
 def test_reader_expression_decimal_is_not_a_scope(self):
  c=checker();c.script(ast('set_variable={which=x value="@[ ( 90 * 0.5 ) ]"}'),'effects',Context('planet','planet'))
  self.assertTrue(all(d['key']=='validation-coverage' for d in c.diagnostics),c.diagnostics)
 def test_dotted_variable_still_resolves_and_rejects_bad_link(self):
  c=checker();c.variable_types['country','stock']='number';c.script(ast('set_variable={which=x value=owner.stock}'),'effects',Context('planet','planet'));self.clean(c)
  c.script(ast('set_variable={which=x value=wrong_scope.stock}'),'effects',Context('planet','planet'));self.error(c,'unknown-scope')
 def test_numeric_missing_script_value_still_errors(self):
  c=checker();c.script(ast('set_variable={which=x value=value:missing|MULT|0.5|}'),'effects',Context('planet','planet'));self.error(c,'missing-item')
 def test_numeric_trigger_scopes_still_checked(self):
  c=checker();c.script(ast('set_variable={which=x value=trigger:planet_size}'),'effects',Context('country','country'));self.error(c,'scope')
 def test_option_icon_background(self):
  c=checker();c.event(ast('country_event={id=t.1 option={icon={icon=GFX_test icon_background=GFX_background}}}')[0]);self.clean(c)
  c.event(ast('country_event={id=t.2 option={icon={icon_background={wrong=yes}}}}')[0]);self.error(c,'structure')
 def test_source_relative_asset_overlay_and_replace_path(self):
  with tempfile.TemporaryDirectory() as t:
   game=Path(t)/'game';mod=Path(t)/'mod';rel='gfx/models/a/local.anim'
   for root in [game,mod]:
    p=root/rel;p.parent.mkdir(parents=True);p.write_bytes(b'animation')
   c=checker();c.asset_roots=[game,mod];c.file=str(game/'gfx/models/a/model.asset');c.asset_reference('local.anim',{});self.clean(c);self.assertIn(str(mod/rel),c.asset_inputs)
   (mod/rel).unlink();c.asset_replace_paths={str(mod):['gfx/models/a/']};c.asset_reference('local.anim',{});self.error(c,'missing-asset')
 def test_asset_shared_texture_and_missing_path(self):
  with tempfile.TemporaryDirectory() as t:
   game=Path(t);p=game/'gfx/models/shared.tga';p.parent.mkdir(parents=True);p.write_bytes(b'tga')
   c=checker();c.asset_roots=[game];c.file=str(game/'gfx/models/sub/model.asset');c.asset_reference('shared.tga',{});self.clean(c)
   c.asset_reference('missing.tga',{});self.error(c,'missing-asset');c.asset_reference('../shared.tga',{});self.error(c,'asset-path')
 def test_explicit_on_action_documentation(self):
  with tempfile.TemporaryDirectory() as t:
   game=Path(t);p=game/'common/on_actions/actions.txt';p.parent.mkdir(parents=True)
   p.write_text('# Root = Planet\n# From = Country\n# FromFrom = Fleet of the science ship, if any\non_planet_surveyed={}\n\n# this = country\non_monthly_pulse_country={}\n\n# This = Country or Planet\non_ambiguous={}\n')
   c=checker();c.prepare_engine_entries(game);ctx=c.engine_contract('on_planet_surveyed','on_actions',{});self.assertEqual(Context('planet','planet',('country','fleet')),ctx);self.assertEqual(['fromfrom'],c.engine_entries[-1]['optional_scopes'])
   c.definition('on_actions',ast('on_planet_surveyed={effect={set_planet_flag=ok from={set_country_flag=ok} fromfrom={set_fleet_flag=ok}}}')[0]);self.clean(c)
   c.definition('on_actions',ast('on_planet_surveyed={effect={set_country_flag=bad}}')[0]);self.error(c,'scope')
   self.assertIsNone(c.engine_contract('on_ambiguous','on_actions',{}));self.assertIsNone(c.engine_contract('on_new_country','on_actions',{}))

if __name__=='__main__':unittest.main()
