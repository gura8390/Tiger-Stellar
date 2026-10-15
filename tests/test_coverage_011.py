import copy,json,sys,unittest,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'src'))
from stellar import Checker,Context
from native_parser import parse_text
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))
def ast(s):return parse_text(s)['ast']
def checker():
 c=Checker(RULES)
 for category,items in {'solar_system_initializers':['fixture_init'],'star_classes':['sc_g'],'planet_classes':['pc_gaia','pc_volcanic'],'buildings':['fixture_building'],'megastructures':['fixture_gate'],'ship_sizes':['starbase_citadel'],'leader_classes':['scientist']}.items():c.entities[category].update(items)
 return c
class Coverage011(unittest.TestCase):
 def script(self,text,scope='galactic_object'):
  c=checker();c.script(ast(text),'effects',Context(scope,scope));return c
 def definition(self,category,text):
  c=checker();c.definition(category,ast('fixture={'+text+'}')[0]);return c
 def assert_clean(self,c):self.assertEqual([],c.diagnostics)
 def assert_error(self,c,key):self.assertTrue(any(x['severity']=='error' and x['key']==key for x in c.diagnostics),c.diagnostics)
 def test_spawn_system_typed_effect_and_parameters(self):
  self.assert_clean(self.script('spawn_system={initializer=fixture_init min_orientation_angle=224 max_orientation_angle=226 hyperlane=no effect={set_star_flag=ok}}'))
  for text,key in [('spawn_system={initializer=missing}','missing-item'),('spawn_system={initializer=fixture_init effect={set_country_flag=bad}}','scope'),('spawn_system={initializer=fixture_init hyperlane=potato}','value'),('spawn_system={initializer=fixture_init min_orientation_agnle=20}','unknown-field'),('spawn_system={initializer=fixture_init min_orientation_angle="!"}','value'),('spawn_system={initializer=fixture_init effect=yes}','structure'),('spawn_system={min_distance=20}','missing-field')]:
   with self.subTest(text=text):self.assert_error(self.script(text),key)
 def test_spawn_system_input(self):self.assert_error(self.script('spawn_system={initializer=fixture_init}','species'),'scope')
 def test_move_system_targets_and_direction(self):
  self.assert_clean(self.script('move_system={target=this direction=rimwards min_distance=55 max_distance=56 hyperlane=no reset_terra_incognita=yes}'))
  for text,key in [('move_system={target=this direction=upwards}','value'),('move_system={target=this directon=rimwards}','unknown-field'),('move_system={direction=rimwards}','missing-field')]:self.assert_error(self.script(text),key)
  self.assert_error(self.script('move_system={target=this}','country'),'scope')
 def test_spawn_megastructure_references(self):
  self.assert_clean(self.script('spawn_megastructure={type=fixture_gate orbit_distance=220 orbit_angle=60}'))
  self.assert_error(self.script('spawn_megastructure={type=missing_gate}'),'missing-item')
  self.assert_error(self.script('spawn_megastructure={type=fixture_gate owner=this}'),'scope')
 def test_native_megastructure_initialization_and_colony_location(self):
  c=checker();c.script(ast('capital_scope={solar_system={spawn_megastructure={type=fixture_gate coords_from=prev init_effect={set_megastructure_flag=ok}}}}'),'effects',Context('country','country'));self.assert_clean(c)
  self.assert_error(self.script('spawn_megastructure={type=fixture_gate init_effect={set_country_flag=bad}}'),'scope')
 def test_global_target_aliases_across_roots(self):
  c=self.script('spawn_system={initializer=fixture_init effect={save_global_event_target_as=home}}')
  c.targets={};c.script(ast('event_target:home={set_star_flag=ok} global_event_target:home={set_star_flag=ok}'),'effects',Context('country','country'));self.assert_clean(c)
  c.script(ast('event_target:home={set_country_flag=bad}'),'effects',Context('country','country'));self.assert_error(c,'scope')
 def test_global_target_conflicting_writers_remain_explicit(self):
  c=checker();c.script(ast('save_global_event_target_as=shared'),'effects',Context('country','country'))
  c.targets={};c.script(ast('save_global_event_target_as=shared'),'effects',Context('galactic_object','country'))
  self.assertEqual('country|galactic_object',c.global_targets['global_event_target:shared'])
  c.targets={};c.script(ast('event_target:shared={set_country_flag=bad}'),'effects',Context('country','country'))
  self.assertTrue(any('polymorphic scope' in x['message'] for x in c.diagnostics))
 def test_unknown_target_still_warns_and_typo_still_errors(self):
  c=self.script('event_target:never_saved={set_star_flag=unknown}')
  self.assertTrue(c.gaps)
  self.assert_error(self.script('event_target:never_saved={set_stra_flag=bad}'),'unknown-effect')
 def test_untyped_global_write_does_not_inherit_a_known_type(self):
  c=self.script('save_global_event_target_as=mixed')
  c.targets={};c.script(ast('save_global_event_target_as=mixed'),'effects',Context('?','country'))
  c.targets={};c.script(ast('event_target:mixed={set_star_flag=unproven}'),'effects',Context('country','country'))
  self.assertTrue(c.gaps)
 def test_initializer_scope_and_refs(self):
  self.assert_clean(self.definition('solar_system_initializers','class=sc_g usage=custom_empire init_effect={set_star_flag=ok} planet={class=star size=30} planet={class=pc_gaia home_planet=yes orbit_distance={min=20 max=30} init_effect={set_planet_flag=ok}}'))
  for body,key in [('class=bad_star','missing-item'),('planet={class=bad_planet}','missing-item'),('planet={class=pc_gaia home_planet=potato}','value'),('planet={class=pc_gaia init_effect={set_country_flag=bad}}','scope'),('init_effect={set_planet_flag=bad}','scope'),('planet={class=pc_gaia orbit_distnace=20}','unknown-field'),('planet=yes','structure'),('usage=unknown_usage','value')]:
   with self.subTest(body=body):self.assert_error(self.definition('solar_system_initializers',body),key)
 def test_bypass_values(self):
  self.assert_clean(self.definition('bypass','is_pathfind=yes connection_type=one_to_one windup_time=0 respect_closed_borders=yes'))
  for body,key in [('connection_type=wrong','value'),('is_pathfind=potato','value'),('windup_time="!"','value'),('connection_typ=one_to_one','unknown-field')]:self.assert_error(self.definition('bypass',body),key)
 def test_native_initializer_variants(self):
  self.assert_clean(self.definition('solar_system_initializers','change_orbit=30 change_orbit={min=60 max=80} planet={class=ideal_design_class starting_planet=yes has_independent_orbital_line=yes satellite_naming_policy=upper_case_roman_numerals}'))
  self.assert_error(self.definition('solar_system_initializers','change_orbit="!"'),'value')
  c=self.definition('solar_system_initializers','planet={class=rl_unknown_selector}')
  self.assertTrue(c.gaps);self.assertFalse(any(d['severity']=='error' for d in c.diagnostics))
 def test_native_job_display_context_and_numeric_weight(self):
  self.assert_clean(self.definition('pop_jobs','swappable_data={swap_type={trigger={is_planet_class=pc_gaia} desc=job_description weight=6}}'))
 def test_building_icon_dds_is_not_a_building_reference(self):
  with tempfile.TemporaryDirectory() as tmp:
   c=checker();c.asset_roots=[Path(tmp)];p=Path(tmp)/'gfx/interface/icons/buildings/legacy_icon.dds';p.parent.mkdir(parents=True);p.write_bytes(b'DDS test metadata')
   c.definition('pop_jobs',ast('job={swappable_data={default={building_icon=legacy_icon}}}')[0]);self.assert_clean(c);self.assertIn(str(p),c.asset_inputs)
   c.definition('pop_jobs',ast('job={swappable_data={default={building_icon=unknown_icon}}}')[0]);self.assert_error(c,'missing-item')
 def test_project_extended_requirements(self):
  c=checker();c.entities['technology'].add('tech_fixture');c.loc.add('EVENT_TOOLTIP')
  c.definition('special_projects',ast('p={requirements={has_technology=tech_fixture handled_by_event_tooltip=EVENT_TOOLTIP}}')[0]);self.assert_clean(c)
  c.definition('special_projects',ast('p={requirements={has_technology=tech_typo}}')[0]);self.assert_error(c,'missing-item')
 def test_megastructure_random_position_and_directions(self):
  self.assert_clean(self.script('spawn_megastructure={type=fixture_gate random_pos=yes}'))
  self.assert_error(self.script('spawn_megastructure={type=fixture_gate random_pos=potato}'),'value')
  for direction in ['rimwards','left','right','in_system']:self.assert_clean(self.script('move_system={target=this direction='+direction+'}'))
 def test_origin_conditions_and_initializer_refs(self):
  self.assert_clean(self.definition('governments/civics','is_origin=yes initializers={fixture_init} starting_colony=pc_gaia playable={always=yes} possible={is_nomadic=no preferred_planet_class={NOT={value=pc_volcanic}}} random_weight={base=0}'))
  for body,key in [('initializers={missing}','missing-item'),('starting_colony=missing','missing-item'),('is_origin=potato','value'),('possible={is_star=yes}','scope'),('possible={preferred_planet_class={value=bad}}','missing-item'),('initializres={fixture_init}','unknown-field')]:self.assert_error(self.definition('governments/civics',body),key)
 def test_job_swappable_data(self):
  self.assert_clean(self.definition('pop_jobs','swappable_data={default={icon=job building_icon=fixture_building} swap_type={trigger={owner={is_ai=no}} name=other}}'))
  for body,key in [('swappable_data={default={building_icon=missing}}','missing-item'),('swappable_data={swap_type={trigger={is_ai=yes}}}','scope'),('swappable_data={default={buidling_icon=fixture_building}}','unknown-field'),('swappable_data=yes','structure')]:self.assert_error(self.definition('pop_jobs',body),key)
 def test_project_requirements(self):
  self.assert_clean(self.definition('special_projects','event_scope=ship_event requirements={shipclass_science_ship=1 leader=scientist}'))
  for body,key in [('requirements={leader=missing}','missing-item'),('requirements={shipclass_science_ship="!"}','value'),('requirements={shipclass_sciense_ship=1}','unknown-field'),('requirements=yes','structure')]:self.assert_error(self.definition('special_projects',body),key)
 def test_native_numeric_frontend(self):
  for scope in ['planet','colony','country','pop_faction']:
   c=checker();c.numeric(ast('value=trigger:pop_amount')[0],Context(scope,scope));self.assert_clean(c)
  for value,scope,key in [('trigger:pop_amunt','planet','unknown-trigger'),('trigger:pop_amount','species','scope'),('trigger:is_ai','country','value')]:
   c=checker();c.numeric(ast('value='+value)[0],Context(scope,scope));self.assert_error(c,key)
  c=checker();c.order_expression(ast('order_by=trigger:pop_amount')[0],Context('planet','planet'));self.assert_clean(c)
 def test_uncalled_script_contract_and_nested_error(self):
  c=checker();node=ast('x={exists=owner owner={has_origin=origin_fixture}}')[0]
  c.entities['governments/civics'].add('origin_fixture');contract=c.infer_script_inputs(node,'triggers');self.assertIn('pop_group',contract.split('|'))
  c.script(node['children'],'triggers',Context(contract,contract));self.assert_clean(c)
  c.script(ast('owner={is_star=yes}'),'triggers',Context(contract,contract));self.assert_error(c,'scope')
 def test_numeric_rule_foreign_scope_rejected(self):
  rules=copy.deepcopy(RULES);rules['numeric_expressions']['bad']={'input':['character'],'numeric':True}
  with self.assertRaisesRegex(ValueError,'wrong-game'):Checker(rules)

 def test_native_spawn_extra_booleans(self):
  self.assert_clean(self.script('spawn_system={initializer=fixture_init authorize_spawn_on_galactic_core=yes} spawn_planet={class=pc_gaia spawn_beyond_gravity_well=yes}'))
  for text in ['spawn_system={initializer=fixture_init authorize_spawn_on_galactic_core=bad}','spawn_planet={class=pc_gaia spawn_beyond_gravity_well=bad}']:self.assert_error(self.script(text),'value')
 def test_native_deposit_selectors(self):
  for value in ['random','random_blocker','random_nonblocker']:self.assert_clean(self.script('add_deposit='+value,'planet'))
  self.assert_error(self.script('add_deposit=random_bloker','planet'),'missing-item')
 def test_native_planet_class_selector(self):
  self.assert_clean(self.script('change_pc=owner_main_species','planet'))
  self.assert_error(self.script('change_pc=owner_main_speciez','planet'),'missing-item')
  self.assert_error(self.script('change_pc=owner_main_species','country'),'scope')
 def test_initializer_empire_neighbor_root(self):
  c=checker();defs={n['key']:n for n in ast('a={usage=fallen_empire_init neighbor_system={initializer=b}} b={neighbor_system={initializer=c}} c={planet={class=pc_gaia init_effect={set_owner=root}}} d={} ')}
  c.index_initializer_roots(defs);self.assertEqual({'a','b','c'},c.empire_initializers)
  c.definition('solar_system_initializers',defs['c']);self.assertFalse(any(d['severity']=='error' for d in c.diagnostics));self.assertTrue(c.gaps)
  self.assert_error(self.definition('solar_system_initializers','planet={class=pc_gaia init_effect={set_owner=root}}'),'scope')

 def test_dynamic_event_target_selector_uses_calling_scope(self):
  c=checker();c.global_targets['global_event_target:exiled_country']='country'
  c.script(ast('owner={is_same_value=event_target:exiled_country@root.controller}'),'triggers',Context('fleet','fleet'))
  self.assertFalse(any(d['severity']=='error' for d in c.diagnostics));self.assertTrue(c.gaps)
  c=checker();c.script(ast('is_same_value=event_target:exiled_country@root.controller'),'triggers',Context('country','country'));self.assert_error(c,'scope')
