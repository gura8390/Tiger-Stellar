import json,sys,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'src'))
from stellar import Checker,Context,children
from native_parser import parse_text
from condition_solver import PotentialProof
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))
def nodes(text):return parse_text(text)['ast']
class Coverage016(unittest.TestCase):
 def checker(self):
  c=Checker(RULES);c.file='fixture.txt';return c
 def proof(self,*conditions):
  c=self.checker();return PotentialProof(c,'hull').prove([nodes('x={potential={'+t+'}}')[0] for t in conditions])['status']
 def check(self,text,kind='triggers',scope='country'):
  c=self.checker();c.events={'a.1':'country'};c.script(nodes(text),kind,Context(scope,scope));return c
 def errors(self,c):return [d for d in c.diagnostics if d['severity']=='error']
 def test_potential_direct_hull(self):self.assertEqual('covered',self.proof('is_ship_size=hull'))
 def test_potential_wrong_hull(self):self.assertEqual('impossible',self.proof('is_ship_size=other'))
 def test_potential_boolean_complement(self):self.assertEqual('covered',self.proof('is_ai=yes','is_ai=no'))
 def test_potential_owner_missing(self):self.assertEqual('unknown',self.proof('owner={is_ai=yes}','owner={is_ai=no}'))
 def test_potential_different_objects(self):self.assertEqual('unknown',self.proof('owner={is_ai=yes}','root={is_ai=no}'))
 def test_potential_unknown_not_proof(self):self.assertEqual('unknown',self.proof('not_documented=yes','not_documented=no'))
 def test_potential_unknown_blocks_not_proof(self):self.assertEqual('unknown',self.proof('opaque={always=yes}','NOT={opaque={always=yes}}'))
 def test_potential_script_expansion(self):
  c=self.checker();c.scripts['triggers']['uses_hull']=(nodes('uses_hull={OR={is_ship_size=hull is_ship_size=other}}')[0],'native.txt')
  result=PotentialProof(c,'hull').prove(nodes('x={potential={uses_hull=yes}}'));self.assertEqual('covered',result['status']);self.assertIn('not proved',result['proof_domain'])
 def test_potential_recursive_script(self):
  c=self.checker();c.scripts['triggers']['recursive']=(nodes('recursive={recursive=yes}')[0],'native.txt')
  self.assertEqual('unknown',PotentialProof(c,'hull').prove(nodes('x={potential={recursive=yes}}'))['status'])
 def test_active_events(self):self.assertFalse(self.check('has_active_event={a.1}').diagnostics)
 def test_active_missing_event(self):self.assertEqual('missing-event',self.errors(self.check('has_active_event={missing.1}'))[0]['key'])
 def test_active_wrong_shape(self):self.assertTrue(self.errors(self.check('has_active_event={id=a.1}')))
 def test_counted_trigger(self):self.assertFalse(self.check('calc_true_if={amount>=2 is_ai=no always=yes}').diagnostics)
 def test_counted_missing_amount(self):self.assertTrue(self.errors(self.check('calc_true_if={always=yes}')))
 def test_counted_wrong_scope(self):self.assertTrue(self.errors(self.check('calc_true_if={amount=1 is_ai=no}',scope='planet')))
 def test_counted_unknown_trigger(self):self.assertTrue(self.errors(self.check('calc_true_if={amount=1 is_aai=no}')))
 def test_failure_tooltip(self):
  c=self.checker();c.loc.add('KEY');c.script(nodes('custom_tooltip_fail={text=KEY is_ai=no}'),'triggers',Context('country','country'));self.assertFalse(c.diagnostics)
 def test_failure_tooltip_wrong_scope(self):self.assertTrue(self.errors(self.check('custom_tooltip_fail={is_ai=no}',scope='planet')))
 def test_research_failure_scope(self):self.assertTrue(self.errors(self.check('add_random_research_option={fail_effects={set_planet_flag=bad}}','effects')))
 def test_message_placeholder(self):self.assertFalse(self.check('create_message={type=UNKNOWN localization=UNKNOWN variable={type=name localization=SYSTEM scope=this}}','effects').gaps)
 def test_message_variable_typo(self):self.assertTrue(self.errors(self.check('create_message={variable={type=name localization=SYSTEM scpoe=this}}','effects')))
 def test_species_callback(self):self.assertFalse(self.check('modify_species={species=this effect={has_trait=missing}}','effects',scope='species').gaps)
 def test_picture_country(self):
  c=self.checker();c.picture_event_data(nodes('picture_event_data={portrait=this room=this}')[0],Context('country','country'));self.assertFalse(c.diagnostics)
 def test_picture_wrong_shape(self):
  c=self.checker();c.picture_event_data(nodes('picture_event_data={portrait={a=b}}')[0],Context('country','country'));self.assertTrue(self.errors(c))
 def test_picture_unknown_contract_stays_gap(self):
  c=self.checker();c.picture_event_data(nodes('picture_event_data={room=this}')[0],Context('species','species'));self.assertTrue(c.gaps)
 def test_macro_not_empty_validated(self):
  c=self.checker();n=nodes('macro={is_ai=$VALUE$}')[0];c.script_contexts[('triggers','macro')]={1:{'context':Context('country','country'),'caller':'a','line':1}}
  self.assertFalse(c.check_script_contexts('triggers','macro',n))
 def test_hull_country_assumption(self):
  c=self.checker();c.definitions['ship_sizes']['hull']=nodes('hull={potential_country={is_ai=no}}')[0]
  self.assertEqual('covered',PotentialProof(c,'hull').prove(nodes('x={potential={is_ship_size=hull from={is_ai=no}}}'))['status'])
  self.assertEqual('impossible',PotentialProof(c,'hull').prove(nodes('x={potential={is_ship_size=hull from={is_ai=yes}}}'))['status'])
 def test_country_unconstrained_not_proof(self):self.assertEqual('unknown',self.proof('from={is_ai=no}'))
 def test_message_literal_variable(self):self.assertFalse(self.check('create_message={variable={key=border value=GFX_border}}','effects').diagnostics)
 def test_message_literal_missing(self):self.assertTrue(self.errors(self.check('create_message={variable={key=border}}','effects')))
 def test_event_flow_root_and_override(self):
  c=self.checker();c.inference_mode=True;c.events['e.1']='country'
  c.remember_event_call('e.1',nodes('country_event={id=e.1}')[0],Context('country','ship',('planet',)))
  c.joined_event_contexts();r=list(c.event_call_evidence['e.1'].values())[0];self.assertEqual(('ship','planet'),r['context'].froms)
  c.next_event_contexts={};c.remember_event_call('e.1',nodes('country_event={id=e.1 scopes={from=this fromfrom=from}}')[0],Context('country','ship',('planet',)))
  self.assertEqual(('country','planet'),list(c.next_event_contexts['e.1'].values())[0]['context'].froms)
 def test_recorded_macro_really_expands(self):
  c=self.checker();c.scripts['triggers']['macro']=(nodes('macro={is_ai=$VALUE$}')[0],'definition.txt');c.inference_mode=True
  c.expand('macro',nodes('macro={VALUE=bad}')[0],'triggers',Context('country','country'));c.inference_mode=False
  self.assertTrue(c.check_script_contexts('triggers','macro',c.scripts['triggers']['macro'][0]));self.assertTrue(self.errors(c))
 def test_per_caller_not_union_validation(self):
  c=self.checker();c.inference_mode=True;c.events['e.1']='country'
  for line,scope in [(1,'country'),(2,'planet')]:c.remember_event_call('e.1',{'key':'country_event','line':line},Context('country',scope))
  c.event_contexts=c.joined_event_contexts();c.inference_mode=False
  self.assertEqual({'country','planet'},{r['context'].froms[0] for r in c.event_variants('e.1','country',True)})
 def test_species_trait_structure(self):self.assertTrue(self.errors(self.check('create_species={traits={bad=missing}}','effects')))
 def test_species_trait_ref(self):self.assertTrue(self.errors(self.check('create_species={traits={trait=missing}}','effects')))
 def test_species_preference_scope(self):self.assertTrue(self.errors(self.check('create_species={traits={ideal_planet_class=this}}','effects')))
 def test_leader_traits_shape(self):self.assertTrue(self.errors(self.check('create_leader={traits={leader_trait_missing}}','effects')))
 def test_leader_traits_reference(self):self.assertTrue(self.errors(self.check('create_leader={traits={0=leader_trait_missing}}','effects')))
 def test_flag_structure(self):self.assertTrue(self.errors(self.check('create_country={flag={colors={null null} icon={category=special}}}','effects')))
 def test_flag_typo(self):self.assertTrue(self.errors(self.check('create_country={flag={colros={null null null null}}}','effects')))
 def test_specimen_shape(self):self.assertTrue(self.errors(self.check('give_specimen={targets={target=this}}','effects')))
 def test_specimen_scope(self):self.assertTrue(self.errors(self.check('give_specimen={targets={this.misspelled}}','effects')))
 def test_weighted_pop_group_scope(self):
  self.assertFalse(self.check('weighted_random_owned_pop_group={limit={owner={is_ai=no}} owner={set_country_flag=ok}}','effects').diagnostics)
  self.assertTrue(self.errors(self.check('weighted_random_owned_pop_group={set_country_flag=bad}','effects')))
 def test_clone_flag_bool(self):self.assertTrue(self.errors(self.check('clone_leader={skip_background_generation=maybe}','effects')))
 def test_flag_scalar_random(self):self.assertFalse(self.check('create_country={flag=random}','effects').diagnostics)
 def test_flag_scalar_source(self):self.assertFalse(self.check('create_country={flag=this}','effects').diagnostics)
 def test_flag_scalar_bad_source(self):self.assertTrue(self.errors(self.check('create_country={flag=this}','effects',scope='planet')))
 def test_event_call_target_variants_not_overwritten(self):
  c=self.checker();c.inference_mode=True;c.events['e.1']='country'
  for target_type in ['country','planet']:
   c.targets={'event_target:selected':target_type};c.remember_event_call('e.1',{'line':1},Context('country','country'))
  c.joined_event_contexts();c.inference_mode=False
  self.assertEqual({'country','planet'},{r['targets']['event_target:selected'] for r in c.event_variants('e.1','country',True)})
 def test_project_root_not_invented(self):
  c=self.checker();c.inference_mode=True;c.events['e.1']='country'
  c.definition('special_projects',nodes('special_project={key=P event_scope=ship_event on_success={owner={country_event={id=e.1}}}}')[0])
  c.joined_event_contexts();self.assertEqual('?',list(c.event_call_evidence['e.1'].values())[0]['context'].froms[0])
 def test_project_root_unknown_stays_coverage(self):
  c=self.checker();c.definition('special_projects',nodes('special_project={key=P event_scope=ship_event on_success={root={set_country_flag=test}}}')[0]);self.assertFalse(self.errors(c));self.assertTrue(c.gaps)
 def test_project_requirement_fleet(self):
  c=self.checker();c.loc.add('KEY');c.definition('special_projects',nodes('special_project={key=P triggered_requirement={text=KEY count=1 trigger={is_ship_class=shipclass_science_ship}}}')[0]);self.assertFalse(c.diagnostics)
 def test_project_requirement_bad_scope(self):
  c=self.checker();c.loc.add('KEY');c.definition('special_projects',nodes('special_project={key=P triggered_requirement={text=KEY trigger={is_ai=no}}}')[0]);self.assertTrue(self.errors(c))
 def test_project_requirement_typo(self):
  c=self.checker();c.definition('special_projects',nodes('special_project={key=P triggered_requirement={coutn=1}}')[0]);self.assertTrue(self.errors(c))
if __name__=='__main__':unittest.main()
