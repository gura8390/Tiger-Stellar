import json,tempfile,unittest,sys,copy,struct
from pathlib import Path
from argparse import Namespace
from test_coverage_011 import checker,ast,ROOT
from stellar import Context,load,run,children,fields
from ship_contracts import hull_condition
from reporting import attribute,diagnostic_delta
from project_reports import ai_possible,constraints,overlay
sys.path.insert(0,str(ROOT/'tools'))
from import_load_order import export

class Status015(unittest.TestCase):
 def clean(self,c):self.assertEqual([],c.diagnostics)
 def error(self,c,key):self.assertTrue(any(d['key']==key and d['severity']=='error' for d in c.diagnostics),c.diagnostics)
 def definition(self,text,kind='weapon_component_template'):
  c=checker();n=ast(kind+'={key=TEST '+text+'}')[0];c.definition('component_templates',n);return c
 def test_damage_order_no_balance_limit(self):
  self.clean(self.definition('damage={min=100000000 max=900000000}total_fire_time=1'))
  self.error(self.definition('damage={min=20 max=10}total_fire_time=1'),'range-order')
 def test_damage_missing_endpoint(self):self.error(self.definition('damage={min=1}total_fire_time=1'),'missing-field')
 def test_damage_wrong_field(self):self.error(self.definition('damage={min=1 maximun=2}total_fire_time=1'),'unknown-field')
 # Installed 4.5.2 component documentation explicitly allows fixed damage=X.
 def test_damage_fixed_scalar(self):self.clean(self.definition('damage=10 total_fire_time=1'))
 def test_damage_invalid_scalar(self):self.error(self.definition('damage="!" total_fire_time=1'),'value')
 def test_damage_numeric(self):self.error(self.definition('damage={min="!" max=2}total_fire_time=1'),'value')
 def test_damage_duplicate(self):self.error(self.definition('damage={min=1 min=2 max=3}total_fire_time=1'),'duplicate-field')
 def test_damage_nonfinite(self):self.error(self.definition('damage={min=nan max=2}total_fire_time=1'),'value')
 def test_native_planet_killer_windup(self):self.clean(self.definition('type=planet_killer damage={min=0 max=0}windup={min=90 max=0}total_fire_time=30'))
 def test_regular_cooldown_wrong(self):self.error(self.definition('damage={min=1 max=2}cooldown=3'),'unknown-field')
 def test_strikecraft_cooldown_correct(self):self.clean(self.definition('damage={min=1 max=2}cooldown=3','strike_craft_component_template'))
 def test_strikecraft_ordinary_timing_wrong(self):self.error(self.definition('damage={min=1 max=2}total_fire_time=3','strike_craft_component_template'),'unknown-field')
 def test_csv_weapon_stats(self):
  c=checker();c.weapon_stats['TEST']={'min_damage':'1','max_damage':'2','cooldown':'3'};c.definition('component_templates',ast('weapon_component_template={key=TEST}')[0]);self.clean(c)
 def test_aura_modifier_and_unknown_field(self):
  c=self.definition('friendly_aura={name=x radius=80 apply_on=ships stack_info={id=x priority=1}modifier={ship_shield_mult=100}}','utility_component_template');self.clean(c)
  self.error(self.definition('friendly_aura={name=x raduis=80}','utility_component_template'),'unknown-field')
 def test_aura_scope(self):self.error(self.definition('hostile_aura={name=x apply_on=ships limit={is_ai=no}}','utility_component_template'),'scope')
 def test_aura_bad_modifier(self):self.error(self.definition('friendly_aura={name=x modifier={misspelled_modifier=1}}','utility_component_template'),'unknown-modifier')
 def test_target_weights_high_valid(self):
  c=checker();c.entities['ship_sizes'].add('target');c.definition('component_templates',ast('utility_component_template={key=TEST target_weights={target=999999999}}')[0]);self.clean(c)
  self.error(self.definition('target_weights={typo=1}','utility_component_template'),'missing-item')
 def setup_ship(self):
  c=checker();defs={'ship_sizes':{'hull':'hull={section_slots={mid={locator=mid}}required_component_set=reactor}'},'section_templates':{'section':'ship_section_template={key=section ship_size=hull fits_on_slot=mid component_slot={name=GUN template=small_turret}}'},'component_slot_templates':{'small_turret':'small_turret={size=small component=weapon}'},'component_templates':{'gun':'weapon_component_template={key=gun size=small power=-5 potential={is_ship_size=hull}}','core':'utility_component_template={key=core size=small component_set=reactor power=10 potential={is_ship_size=hull}}'}}
  for cat,items in defs.items():
   for key,text in items.items():c.definitions[cat][key]=dict(ast(text)[0],_source_path='fixture.txt');c.entities[cat].add(key)
  return c
 def design(self,c,text=''):
  n=ast('ship_design={name=design ship_size=hull section={slot=mid template=section component={slot=GUN template=gun}}required_component=core '+text+'}')[0];c.design_contract(n);return c
 def test_explicit_design_compatible(self):
  c=self.design(self.setup_ship());self.clean(c);self.assertEqual(5,c.ship_feasibility[0]['net_component_power'])
 def test_explicit_design_wrong_component_size(self):
  c=self.setup_ship();fields(c.definitions['component_templates']['gun'])['size']['value']='large';self.error(self.design(c),'ship-compatibility')
 def test_explicit_design_wrong_component_type(self):
  c=self.setup_ship();c.definitions['component_templates']['gun']['key']='utility_component_template';self.error(self.design(c),'ship-compatibility')
 def test_native_strike_craft_and_any_slots(self):
  c=self.setup_ship();c.definitions['component_templates']['gun']['key']='strike_craft_component_template';fields(c.definitions['component_slot_templates']['small_turret'])['component']['value']='strike_craft';self.clean(self.design(c))
  c=self.setup_ship();fields(c.definitions['component_slot_templates']['small_turret'])['component']['value']='any';fields(c.definitions['component_slot_templates']['small_turret'])['size']['value']='any';self.clean(self.design(c))
 def test_retains_multiple_call_paths(self):
  c=checker();c.scripts['effects']['wrong']=(ast('wrong={set_country_flag=bad}')[0],'effects.txt')
  for i in [1,2]:c.definition('events',ast('fleet_event={id=test.'+str(i)+' immediate={wrong=yes}}')[0])
  self.assertEqual(1,len(c.diagnostics));self.assertEqual(2,len(c.diagnostics[0]['call_paths']))
 def test_partial_design_power_is_unknown(self):
  c=self.setup_ship();c.design_contract(ast('ship_design={name=partial ship_size=hull section={slot=mid template=section component={slot=GUN template=gun}}}')[0]);self.assertFalse(any(d['key']=='ship-power' for d in c.diagnostics));self.assertEqual('unknown',c.ship_feasibility[0]['status'])
 def test_nonfinite_power_rejected(self):self.error(self.definition('power=nan','utility_component_template'),'value')
 def test_design_missing_section(self):
  c=self.setup_ship();del c.definitions['section_templates']['section'];c.entities['section_templates'].clear();self.error(self.design(c),'missing-item')
 def test_core_wrong_hull(self):
  c=self.setup_ship();fields(fields(c.definitions['component_templates']['core'])['potential'])['is_ship_size']['value']='other';self.error(self.design(c),'ship-compatibility')
 def test_core_wrong_set(self):
  c=self.setup_ship();fields(c.definitions['component_templates']['core'])['component_set']['value']='other';self.error(self.design(c),'ship-compatibility')
 def test_power_deficit(self):
  c=self.setup_ship();fields(c.definitions['component_templates']['core'])['power']['value']='1';self.error(self.design(c),'ship-power')
 def test_unknown_eligibility_not_success(self):
  c=self.setup_ship();fields(c.definitions['component_templates']['core'])['potential']['children'].append(ast('from={is_ai=no}')[0]);self.design(c);self.assertEqual('unknown',c.ship_feasibility[0]['status'])
 def test_upgrade_cycle(self):
  c=self.setup_ship();c.definitions['component_templates']['gun']['children'].append(ast('upgrades_to=gun')[0]);c.ship_crosschecks([('fixture.txt','component_templates',[c.definitions['component_templates']['gun']])]);self.error(c,'upgrade-cycle')
 def test_hull_tri_state(self):
  self.assertIs(False,hull_condition(ast('OR={is_ship_size=other is_ship_size=another}'),'hull'));self.assertIs(None,hull_condition(ast('OR={is_ship_size=other from={is_ai=no}}'),'hull'))
 def test_engine_entry_not_guessed_root(self):
  c=self.setup_ship();c.definitions['component_templates']['K']=ast('weapon_component_template={key=K type=planet_killer}')[0];c.entry_sources={'planet_killer_trigger':{'path':'documented.txt'}}
  ctx=c.engine_contract('can_destroy_planet_with_K','triggers',{});self.assertEqual(('planet','?',('fleet',)),(ctx.current,ctx.root,ctx.froms))
  self.assertIsNone(c.engine_contract('can_destroy_planet_with_typo','triggers',{}))
  self.assertIsNone(c.engine_contract('can_destroy_planet_with_core','triggers',{}))
  c.script(ast('is_ai=no'),'triggers',ctx);self.error(c,'scope')
 def test_engine_entry_fleet_and_planet(self):
  c=checker();c.script(ast('is_star=yes from={owner={is_ai=no}}'),'triggers',Context('planet','?',('fleet',)));self.clean(c)
  c=checker();c.script(ast('from={is_ai=no}'),'triggers',Context('planet','?',('fleet',)));self.error(c,'scope')
 def test_graphics_refs_and_dds(self):
  with tempfile.TemporaryDirectory() as t:
   c=checker();c.asset_roots=[Path(t)];p=Path(t)/'icon.dds';data=bytearray(128);data[:4]=b'DDS ';struct.pack_into('<4I',data,4,124,0,4,4);struct.pack_into('<I',data,76,32);p.write_bytes(data)
   c.asset_reference('icon.dds',{});self.clean(c);p.write_bytes(b'DDS ');c.asset_reference('icon.dds',{});self.error(c,'dds-header')
   c.asset_reference('missing.dds',{});self.error(c,'missing-asset')
 def test_graphics_missing_entity(self):
  c=checker();c.graphics(ast('entity={name=test pdxmesh=misspelled}'));self.error(c,'missing-item')
 def test_resource_root_escape(self):
  c=checker();c.asset_reference('../external.dds',{});self.error(c,'asset-path')
 def test_name_lists_bad_structure(self):
  c=checker();c.name_list(ast('TEST={character_names={default={full_names_male={wrong=block}}}}')[0]);self.error(c,'name-list-structure')
 def test_language_does_not_fallback(self):
  c=checker();c.loc={'KEY'};c.loc_languages={'english':{'KEY'},'simp_chinese':set()};c.language='simp_chinese';c.localisation({'value':'KEY'});self.assertTrue(c.diagnostics)
 def test_duplicate_localization_and_file_overlay(self):
  with tempfile.TemporaryDirectory() as t:
   base=Path(t)/'game';mod=Path(t)/'mod'
   for root in [base,mod]:(root/'localisation/simp_chinese').mkdir(parents=True)
   (base/'localisation/simp_chinese/a.yml').write_text('l_simp_chinese:\n OLD:0 "old"',encoding='utf8')
   (mod/'localisation/simp_chinese/a.yml').write_text('l_simp_chinese:\n NEW:0 "new"\n NEW:0 "again"',encoding='utf8')
   c=checker();c.language='simp_chinese';c.selected_localisation([base,mod]);self.assertNotIn('OLD',c.loc);self.assertTrue(any(x['key']=='duplicate-localization' for x in c.diagnostics))
 def test_policy_boolean_not_text_search(self):
  self.assertIs(False,ai_possible(ast('is_ai=no')));self.assertIs(None,ai_possible(ast('OR={is_ai=no unknown_condition=yes}')))
  self.assertIs(True,ai_possible(ast('NOT={is_ai=no}')));self.assertIs(False,ai_possible(ast('owner={is_ai=no}'),('owner',)))
 def test_policy_construction_not_benefits(self):
  c=checker();n=ast('building={potential={owner={is_ai=no}}resources={produces={energy=20}}}')[0];c.definitions['buildings']['building']=dict(n,_source_path='fixture.txt')
  r=constraints(c,{'schema_version':1,'player_only':[{'category':'buildings','definition':'building','checks':[{'purpose':'construction','field_path':['potential'],'actor_path':['owner']},{'purpose':'benefits','field_path':['resources','produces','trigger'],'actor_path':['owner']}]}]})
  self.assertIn('excluded',r[0]['status']);self.error(c,'project-constraint')
 def test_policy_checks_all_repeated_branches(self):
  c=checker();n=ast('b={triggered_planet_modifier={potential={always=yes}}triggered_planet_modifier={potential={owner={is_ai=no}}}}')[0];c.definitions['buildings']['b']=dict(n,_source_path='fixture.txt')
  r=constraints(c,{'schema_version':1,'player_only':[{'category':'buildings','definition':'b','checks':[{'purpose':'benefits','field_path':['triggered_planet_modifier','potential'],'actor_path':['owner']}]}]});self.assertEqual(2,r[0]['branches_checked']);self.error(c,'project-constraint')
 def test_attribution_line_moves_and_real_change(self):
  current=checker();control=checker()
  base=ast('country_event={id=test.1 trigger={is_star=yes}}')[0]
  copy_node=ast('\n\ncountry_event={id=test.1 trigger={is_star=yes}}')[0]
  control.file='D:/game/events/test.txt';current.file='D:/mod/events/test.txt'
  control.definitions['events']['test.1']=dict(base,_source_path=control.file);current.definitions['events']['test.1']=dict(copy_node,_source_path=current.file)
  control.definition('events',base);current.definition('events',copy_node)
  attribute(current,control,Path('D:/game'),[Path('D:/mod')]);self.assertEqual('vanilla-existing',current.diagnostics[0]['attribution']['classification'])
  changed=checker();changed.file=current.file;changed.definition('events',ast('country_event={id=test.1 trigger={has_planet_flag=modified}}')[0]);attribute(changed,control,Path('D:/game'),[Path('D:/mod')]);self.assertEqual('mod-new-or-modified',changed.diagnostics[0]['attribution']['classification'])
 def test_diagnostic_delta_rejects_changed_hash(self):
  a={'input_manifest':[{'path':'a','sha256':'1'}]};b={'input_manifest':[{'path':'a','sha256':'2'}]};self.assertFalse(diagnostic_delta(a,b)['comparable'])
 def test_dependency_order_overlay(self):
  with tempfile.TemporaryDirectory() as t:
   roots=[Path(t)/x for x in ['game','A','B']]
   for i,r in enumerate(roots):
    (r/'common/ship_sizes').mkdir(parents=True);(r/'common/ship_sizes/same.txt').write_text('hull={max_speed='+str(i)+'}',encoding='utf8')
   for mods in [roots[1:],list(reversed(roots[1:]))]:
    c=checker();load(c,roots[0],mods);o=overlay(c,roots[0],mods);record=next(x for x in o['effective_definitions'] if x['name']=='hull');self.assertTrue(Path(record['provider']).is_relative_to(mods[-1]))
 def test_load_list_export_order_and_no_write(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t);(root/'mod').mkdir();p=root/'dlc_load.json'
   for name in ['A','B']:(root/name).mkdir();(root/'mod'/f'{name}.mod').write_text(f'path="{name}"',encoding='utf8')
   text=json.dumps({'enabled_mods':['mod/A.mod','mod/B.mod']});p.write_text(text,encoding='utf8');r=export(p,root);self.assertEqual(['--dependency',str(root/'A'),'--mod',str(root/'B')],r['arguments']);self.assertEqual(text,p.read_text())

if __name__=='__main__':unittest.main()
