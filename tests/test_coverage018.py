import json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'src'))
from stellar import Checker,Context,UNKNOWN
from native_parser import parse_text
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))
def ast(t):return parse_text(t)['ast']
class Coverage018(unittest.TestCase):
 def c(self):
  c=Checker(RULES);c.file='fixture.txt';return c
 def check(self,t,scope='country|planet'):
  c=self.c();c.script(ast(t),'triggers',Context(scope,scope));return c
 def errors(self,c):return [d for d in c.diagnostics if d['severity']=='error']
 def test_implicit_guard(self):self.assertFalse(self.check('is_scope_type=country is_ai=no').diagnostics)
 def test_wrong_guard(self):self.assertTrue(self.errors(self.check('is_scope_type=planet is_ai=no')))
 def test_or_not_conjunction(self):self.assertTrue(self.check('OR={is_scope_type=country is_ai=no}').gaps)
 def test_or_typed_arms(self):self.assertFalse(self.check('OR={AND={is_scope_type=country is_ai=no} AND={is_scope_type=planet is_star=yes}}').diagnostics)
 def test_not_guard(self):self.assertFalse(self.check('NOT={is_scope_type=planet} is_ai=no').diagnostics)
 def scripted(self,body,condition):
  c=self.c();c.scripts['triggers']['guard']=(ast('guard={'+body+'}')[0],'guard.txt');return c,c.narrow(ast(condition),Context('country|planet','country'))
 def test_script_guard_yes(self):self.assertEqual('country',self.scripted('is_scope_type=country','guard=yes')[1].current)
 def test_script_guard_no(self):self.assertEqual('planet',self.scripted('is_scope_type=country','guard=no')[1].current)
 def test_script_mixed_negative_unknown(self):self.assertEqual('country|planet',self.scripted('is_scope_type=country is_ai=no','guard=no')[1].current)
 def test_script_input_not_guard(self):self.assertEqual('country|planet',self.scripted('is_ai=no','guard=yes')[1].current)
 def test_recursive_guard_unknown(self):self.assertEqual('country|planet',self.scripted('guard=yes','guard=yes')[1].current)
 def test_effects_not_narrowed(self):
  c=self.c();c.script(ast('set_country_flag=ok'),'effects',Context('country|planet','country'));self.assertTrue(c.gaps)
 def entry(self,t,ctx=None):
  c=self.c();c.events={'a.1':'country'};c.inference_mode=True;c.on_action_events(ast(t)[0],ctx or Context('country','country',('planet','ship')));c.inference_mode=False;c.on_action_events(ast(t)[0],ctx or Context('country','country',('planet','ship')));return c
 def test_entry_preserves_from(self):
  c=self.entry('events={a.1}');r=next(iter(c.next_event_contexts['a.1'].values()));self.assertEqual(('planet','ship'),r['context'].froms)
 def test_entry_weighted_preserves_from(self):self.assertIn('a.1',self.entry('random_events={10=a.1 5=0}').next_event_contexts)
 def test_entry_zero_not_dispatched(self):self.assertFalse(self.entry('random_events={0=a.1}').next_event_contexts)
 def test_entry_unknown_reference(self):self.assertTrue(self.errors(self.entry('events={missing.1}')))
 def test_entry_zero_missing_reference(self):self.assertTrue(self.errors(self.entry('random_events={0=missing.1}')))
 def test_entry_bad_weight(self):self.assertTrue(self.errors(self.entry('random_events={-1=a.1}')))
 def test_entry_nonfinite_weight(self):self.assertTrue(self.errors(self.entry('random_events={1e999=a.1}')))
 def test_entry_bad_shape(self):self.assertTrue(self.errors(self.entry('events={id=a.1}')))
 def test_entry_nested_shape(self):self.assertTrue(self.errors(self.entry('random_events={10={id=a.1}}')))
 def test_entry_unknown_from(self):
  c=self.entry('events={a.1}',Context(UNKNOWN,UNKNOWN));self.assertFalse(next(iter(c.next_event_contexts['a.1'].values()))['context'].froms)
 def test_entry_mismatch_loses_preset(self):
  c=self.entry('events={a.1}',Context('planet','planet',('ship',)));self.assertFalse(next(iter(c.next_event_contexts['a.1'].values()))['context'].froms)
 def picture(self,text,scope):
  c=self.c();c.entities['gfx_portrait'].add('known');c.picture_event_data(ast('picture_event_data={'+text+'}')[0],Context(scope,scope));return c
 def test_portrait_leader(self):self.assertFalse(self.picture('portrait=this','leader').diagnostics)
 def test_background_planet(self):self.assertFalse(self.picture('planet_background=this','planet').diagnostics)
 def test_portrait_literal(self):self.assertFalse(self.picture('portrait=known','country').diagnostics)
 def test_portrait_missing(self):self.assertTrue(self.errors(self.picture('portrait=missing','country')))
 def test_portrait_unsupported_stays_gap(self):self.assertTrue(self.picture('portrait=this','species').gaps)
 def test_portrait_bad_shape(self):self.assertTrue(self.errors(self.picture('portrait={x=y}','country')))
 def test_room_missing(self):self.assertTrue(self.errors(self.picture('room=missing','country')))
 def agreement(self,key='joins',value='none',term='joins'):
  c=self.c();c.entities['agreement_terms'].add('joins');c.entities['agreement_term_values'].add('none');c.definitions['agreement_term_values']['none']=ast('none={term='+term+'}')[0];c.extended_child('set_agreement_terms',ast(key+'='+value)[0],Context('country','country'));return c
 def test_agreement_valid(self):self.assertFalse(self.agreement().diagnostics)
 def test_agreement_missing(self):self.assertTrue(self.errors(self.agreement(value='missing')))
 def test_agreement_wrong_term(self):self.assertTrue(self.errors(self.agreement(term='other')))
 def test_agreement_shape(self):self.assertTrue(self.errors(self.agreement(value='{}')))
 def country_name(self,text):
  c=self.c();c.loc.add('KEY');c.extended_child('create_country',ast('name={'+text+'}')[0],Context('country','country'));return c
 def test_country_name_valid(self):self.assertFalse(self.country_name('key=KEY variable_string="[Runtime.GetName]"').diagnostics)
 def test_country_name_missing_key(self):self.assertTrue(self.errors(self.country_name('variable_string=x')))
 def test_country_name_unknown_grammar(self):self.assertTrue(self.country_name('key=KEY undocumented=x').gaps)
 def project(self,text):
  c=self.c();c.loc.add('KEY');c.declaration_subgrammar('special_projects',ast('desc={'+text+'}')[0],Context('ship','ship'));return c
 def test_project_desc_ship(self):self.assertFalse(self.project('text=KEY trigger={is_ship_class=shipclass_science_ship}').diagnostics)
 def test_project_desc_wrong_scope(self):self.assertTrue(self.errors(self.project('text=KEY trigger={is_ai=no}')))
 def test_project_desc_typo(self):self.assertTrue(self.errors(self.project('text=KEY trigger={is_aai=no}')))
 def test_project_root_unknown(self):self.assertTrue(self.project('text=KEY trigger={root={is_ai=no}}').gaps)
 def test_join_missing_from_stays_unknown(self):
  c=self.c();c.next_event_contexts={'a.1':{1:{'context':Context('country','country',('planet',)),'targets':{},'caller':'a','line':1},2:{'context':Context('country','country'),'targets':{},'caller':'b','line':2}}}
  self.assertEqual((UNKNOWN,),c.joined_event_contexts()['a.1']['merged']['context'].froms)
 def test_join_known_from_types(self):
  c=self.c();c.next_event_contexts={'a.1':{1:{'context':Context('country','country',('planet',)),'targets':{},'caller':'a','line':1},2:{'context':Context('country','country',('ship',)),'targets':{},'caller':'b','line':2}}}
  self.assertEqual(('planet|ship',),c.joined_event_contexts()['a.1']['merged']['context'].froms)
if __name__=='__main__':unittest.main()
