import json,tempfile,unittest
from pathlib import Path
from test_coverage_011 import checker,ast
from stellar import Context,UNKNOWN
from native_parser import explain_braces

class Handoff031(unittest.TestCase):
 def check_script(self,text,scope='country',kind='triggers',root=None,froms=()):
  c=checker();c.entities['component_templates'].add('component_ok');c.entities['megastructures'].add('mega_ok');c.script(ast(text),kind,Context(scope,root or scope,froms));return c
 def clean(self,c):self.assertEqual([],c.diagnostics)
 def error(self,c,key):self.assertTrue(any(d['severity']=='error' and d['key']==key for d in c.diagnostics),c.diagnostics)
 def definition(self,category,text):
  c=checker();c.entities['ship_sizes'].add('next_ship');c.entities['component_templates'].add('component_only');c.entities['strategic_resources'].add('energy');c.definition(category,ast('fixture={'+text+'}')[0]);return c
 def test_upgrade_is_ship_reference(self):
  self.clean(self.definition('ship_sizes','upgrades_to=next_ship'))
  for name in ['component_only','misspelled_ship']:self.error(self.definition('ship_sizes','upgrades_to='+name),'missing-item')
 def test_and_scope_guard(self):
  self.clean(self.check_script('AND={is_scope_type=megastructure is_megastructure_type=mega_ok}','country|megastructure'))
  self.error(self.check_script('is_megastructure_type=mega_ok'),'scope')
 def test_if_else_type_refinement(self):
  self.clean(self.check_script('if={limit={is_scope_type=megastructure}is_megastructure_type=mega_ok}else={is_ai=no}','country|megastructure'))
  self.error(self.check_script('if={limit={is_scope_type=country}is_megastructure_type=mega_ok}','country|megastructure'),'scope')
 def test_or_guard_does_not_exclude_unrelated_alternative(self):
  c=self.check_script('AND={OR={is_scope_type=megastructure always=yes}is_megastructure_type=mega_ok}','country|megastructure');self.assertTrue(c.gaps)
 def test_false_mixed_limit_does_not_invent_type(self):
  c=checker();ctx=Context('country|megastructure','country');n=c.narrow(ast('is_scope_type=megastructure always=yes'),ctx,False);self.assertEqual(ctx.current,n.current)
 def test_or_of_two_type_guards(self):
  c=checker();ctx=Context('country|megastructure|planet','country');n=c.narrow(ast('OR={is_scope_type=country is_scope_type=megastructure}'),ctx);self.assertEqual({'country','megastructure'},set(n.current.split('|')))
 def test_federation_and_country_relative_power(self):
  for scope in ['country','federation']:self.clean(self.check_script('relative_power={who=this value>equivalent category=fleet}',scope))
  self.error(self.check_script('relative_power={who=from value>equivalent category=fleet}',froms=('planet',)),'scope')
  self.error(self.check_script('relative_power={who=this value>equivalant category=fleet}'),'value')
 def test_ship_resource_trigger_and_builder_from(self):
  self.clean(self.definition('ship_sizes','resources={cost={trigger={is_ship_size=next_ship from={is_ai=no}}energy=1}}'))
  self.error(self.definition('ship_sizes','resources={cost={trigger={is_ai=no}energy=1}}'),'scope')
 def test_component_context(self):
  for scope in ['ship','ship_growth_stage']:self.clean(self.check_script('has_component=component_ok',scope))
  self.error(self.check_script('has_component=component_ok'),'scope')
  self.error(self.check_script('has_component=component_typo','ship'),'missing-item')
 def test_root_and_iteration_preserved(self):
  c=checker();c.entities['ethics'].add('ethic_fixture');c.script(ast('any_country={root={has_ethic=ethic_fixture}}'),'triggers',Context('country','country'));self.clean(c)
  c=checker();c.entities['ethics'].add('ethic_fixture');c.script(ast('any_country={root={has_ethic=ethic_fixture}}'),'triggers',Context('none','none'));self.error(c,'scope')
  c=checker();c.entities['ethics'].add('ethic_fixture');c.event(ast('event={id=test.1 immediate={every_country={if={limit={root={has_ethic=ethic_fixture}}set_country_flag=ok}}}}')[0]);self.assertFalse(any(d['severity']=='error' for d in c.diagnostics));self.assertTrue(c.gaps)
  c=checker();c.event(ast('event={id=test.1 immediate={set_country_flag=illegal_this}}')[0]);self.error(c,'scope')
 def test_civic_modification_discovers_call(self):
  c=checker();c.scripts['triggers']['reform']=(ast('reform={is_ai=no}')[0],'reform.txt');c.definition('governments/civics',ast('fixture={modification={add={reform=yes}remove={reform=yes}}}')[0]);self.clean(c);self.assertIn(('triggers','reform'),c.invoked)
  self.error(self.definition('governments/civics','modification={add={is_ship_size=next_ship}}'),'scope')
 def test_modification_boolean(self):
  self.clean(self.definition('governments/civics','modification=no'));self.error(self.definition('governments/civics','modification=potato'),'value')
 def test_section_slots_and_ai(self):
  self.clean(self.definition('ship_sizes','section_slots={mid={locator=root}}ai_ship_data={fraction={base=1 modifier={factor=2 is_ai=no}}planet_mult=0.1}'))
  self.error(self.definition('ship_sizes','section_slots={mid={locater=root}}'),'unknown-field')
 def test_diplomatic_actor_recipient(self):
  self.clean(self.definition('diplomatic_actions','potential={is_ai=no from={is_ai=yes}}on_accept={from={set_country_flag=ok}}ai_acceptance={base=1 modifier={add=1 is_ai=no}}'))
  self.error(self.definition('diplomatic_actions','potential={from={is_ship_size=next_ship}}'),'scope')
 def test_country_modules_validate_actions(self):
  c=checker();c.entities['diplomatic_actions'].add('known');c.definition('country_types',ast('x={modules={exclusive_diplomacy_module={can_send={known}}}}')[0]);self.clean(c)
  self.error(self.definition('country_types','modules={exclusive_diplomacy_module={can_send={unknown_action}}}'),'missing-item')
 def test_personality_conditions(self):
  self.clean(self.definition('personalities','aggressiveness=0.5 allow={is_ai=yes}weight_modifier={base=1}'))
  self.error(self.definition('personalities','allow={is_ship_size=next_ship}'),'scope')
 def test_project_callbacks_have_distinct_stacks(self):
  c=checker();c.project_locations['fixture']='starbase';c.definition('special_projects',ast('fixture={event_scope=ship_event on_start={set_ship_flag=ok prev={set_country_flag=ok}from={solar_system={set_star_flag=ok}}}on_cancel={set_country_flag=ok from={set_ship_flag=ok}fromfrom={solar_system={set_star_flag=ok}}}}')[0]);self.clean(c)
  self.error(self.definition('special_projects','event_scope=ship_event on_cancel={set_ship_flag=bad}'),'scope')
 def test_project_enable_records_location(self):
  c=checker();c.entities['special_projects'].add('fixture');c.script(ast('enable_special_project={name=fixture location=this}'),'effects',Context('starbase','starbase'));self.assertEqual('starbase',c.project_locations['fixture'])
 def test_numeric_export_type_without_runtime_value(self):
  c=checker();c.entities['strategic_resources'].add('energy');ctx=Context('country','country');c.script(ast('export_resource_stockpile_to_variable={resource=energy variable=stock}set_variable={which=copy value=stock}'),'effects',ctx);self.clean(c)
  c.numeric(ast('n=copy')[0],ctx);self.clean(c)
  c.numeric(ast('n=never_assigned')[0],ctx);self.assertTrue(c.gaps)
 def test_unknown_numeric_modifier_is_error(self):
  c=checker();c.numeric(ast('x=modifier:nonexistent_modifier')[0],Context('country','country'));self.error(c,'unknown-modifier')
 def test_conditional_picture(self):
  c=checker();c.event(ast('country_event={id=fixture picture={trigger={is_ai=no}picture=GFX_known}}')[0]);self.clean(c)
  c=checker();c.event(ast('country_event={id=fixture picture={trigger={is_ship_size=fixture}picture=GFX_known}}')[0]);self.error(c,'scope')
 def test_flag_and_government_parameters(self):
  c=checker();c.entities['flag_colors'].add('blue');c.entities['governments/civics'].add('civic_ok');c.script(ast('change_country_flag={icon={category=test file=test}background={category=test file=test}colors={blue null}}change_government={civics={civic=civic_ok}cooldown=no}'),'effects',Context('country','country'));self.clean(c)
  self.error(self.check_script('change_government={civics={civic=misspelled}}',kind='effects'),'missing-item')
 def test_brace_heuristic_explains_actual_opener(self):
  text='outer={\n inner={\n}\n}\n';item={'diagnostics':[{'line':3,'column':1,'key':'brace-placement'}]};d=explain_braces(item,text)['diagnostics'][0];self.assertIn('2:8',d['message']);self.assertEqual('formatting_suspicion',d['classification'])
 def test_starbase_iterator_type(self):
  c=checker();self.assertEqual('starbase',c.rules['triggers']['any_starbase_in_system']['iterator_output'])
 def test_empty_parent_and_metadata_are_not_validation_success(self):
  from argparse import Namespace
  from stellar import run
  from native_parser import resource_root
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);game=root/'game';game.mkdir();(game/'launcher-settings.json').write_text(json.dumps({'gameId':'stellaris','rawVersion':'v4.5.2'}))
   for name,state in [('parent','parent-directory'),('metadata','metadata-assets-only')]:
    mod=root/name;mod.mkdir();(mod/'descriptor.mod').write_text('name="Metadata"')
    if name=='parent':(mod/'mod/common').mkdir(parents=True)
    output=root/(name+'.json');code=run(Namespace(game=game,mod=mod,dependency=[],cache=None,syntax_only=False,rules=resource_root()/'rules/stellaris-4.5.2.json',output=output));r=json.loads(output.read_text())
    self.assertEqual(2,code);self.assertEqual(0,r['target_files']);self.assertEqual(state,r['input_state']);self.assertEqual('empty_input_not_validation',r['status'])
 def test_nested_component_inline_registers_identifier(self):
  from stellar import load
  with tempfile.TemporaryDirectory() as tmp:
   game=Path(tmp);p=game/'common/inline_scripts/component.txt';p.parent.mkdir(parents=True);p.write_text('key="$KEY$" size=small')
   p=game/'common/component_templates/test.txt';p.parent.mkdir(parents=True);p.write_text('utility_component_template={inline_script={script=component KEY=COMPONENT_GENERATED}}')
   c=checker();load(c,game,[]);self.assertIn('COMPONENT_GENERATED',c.entities['component_templates'])
 def test_malformed_inline_remains_parser_error(self):
  with tempfile.TemporaryDirectory() as tmp:
   p=Path(tmp)/'bad.txt';p.write_text('set_country_flag=ok }');c=checker();c.inline['bad']=p;c.script(ast('inline_script=bad'),'effects',Context('country','country'));self.assertTrue(any(d['severity']=='error' for d in c.diagnostics))
 def test_inline_inherits_caller_reader_variables(self):
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);caller=root/'caller.txt';caller.write_text('@local_days=80\n');inline=root/'inline.txt';inline.write_text('set_timed_country_flag={flag=ok days=@local_days}')
   c=checker();c.globals_text='@local_days=1';c.file=str(caller);c.inline['inline']=inline;c.script(ast('inline_script=inline'),'effects',Context('country','country'));self.clean(c)
 def test_misspelled_scope_filter_is_error(self):
  self.error(self.check_script('is_scope_type=character'),'value')
 def test_nested_logic_preserves_if_else_chain(self):
  self.clean(self.check_script('AND={if={limit={is_scope_type=megastructure}is_megastructure_type=mega_ok}else={is_ai=no}}','country|megastructure'))
 def test_iterator_preserves_unreachable_else(self):
  self.clean(self.check_script('every_country={if={limit={is_scope_type=country}set_country_flag=ok}else={set_ship_flag=bad}}',kind='effects'))
 def test_event_effect_list_preserves_unreachable_else(self):
  c=checker();c.event(ast('country_event={id=test.1 immediate={if={limit={is_scope_type=country}set_country_flag=ok}else={set_ship_flag=bad}}}')[0]);self.clean(c)

 def test_job_name_does_not_authorize_undocumented_modifier_suffix(self):
  from stellar import load
  with tempfile.TemporaryDirectory() as tmp:
   root=Path(tmp);p=root/'common/pop_jobs/fixture.txt';p.parent.mkdir(parents=True);p.write_text('made_up_job={}')
   c=checker();load(c,root,[]);c.modifier(ast('modifier={made_up_job_jobs_bonus_workforce_mult=1}')[0],Context('country','country'));self.error(c,'unknown-modifier')

if __name__=='__main__':unittest.main()
