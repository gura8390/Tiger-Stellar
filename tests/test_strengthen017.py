import collections,json,sys,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'src'))
from stellar import Checker,Context,children
from native_parser import parse_text
from diagnostic_kinds import classify,infer_kind,summary,KINDS
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))
def ast(text):return parse_text(text)['ast']
class Strengthen017(unittest.TestCase):
 def c(self):
  c=Checker(RULES);c.file='fixture.txt';c.entities['situations'].add('known');c.targets['event_target:dest']='planet';return c
 def run_script(self,text,scope='fleet'):
  c=self.c();c.script(ast(text),'effects',Context(scope,scope));return c
 def errors(self,c):return [d for d in c.diagnostics if d['severity']=='error']
 def test_reason_does_not_change_diagnostic(self):
  d={'path':'a','line':1,'column':2,'key':'validation-coverage','severity':'warning','message':'scope cannot be inferred for owner'};before=dict(d);classify(d)
  self.assertEqual(before,{k:d[k] for k in before});self.assertEqual('scope-type-unknown',d['reason']['code']);self.assertFalse(d['reason']['game_code_fault_proven'])
 def test_legacy_reason_mapping(self):
  for text,kind in [('recursive scripted call: x','analysis-limit'),('scripted definition has no inferred caller context: x','caller-context-unknown'),('dynamic event target identity cannot be inferred: x','dynamic-target-identity'),('numeric variable cannot be resolved statically: x','numeric-information-unresolved'),('parameter subgrammar not specified: x','rule-unimplemented'),('required set x potential rejects y; fixed/generated design bypass contract is not verified','engine-contract-unverified'),('polymorphic scope planet|country for x','scope-type-ambiguous'),('new unspecified wording','unclassified-review')]:
   self.assertEqual(kind,infer_kind('validation-coverage',text,'warning'))
 def test_formatting_is_not_syntax_error(self):self.assertEqual('formatting-suspicion',infer_kind('brace-placement','possible brace error','warning'))
 def test_error_still_error(self):self.assertEqual('confirmed-rule-violation',infer_kind('scope','wrong scope','error'))
 def test_summary_has_defined_denominator(self):
  c=self.c();c.gap('scope cannot be inferred for owner');c.gap('scope cannot be inferred for owner');c.gap('parameter subgrammar not specified: X',{'line':2});rows=summary(c.diagnostics)
  self.assertEqual(len(c.diagnostics),sum(g['count'] for g in rows));self.assertEqual(2,len(c.diagnostics));self.assertEqual(2,c.gaps['scope cannot be inferred for owner'])
 def test_scope_reason_has_contract_evidence(self):
  c=self.c();c.compatible('planet|country',['country'],{},'operation');d=c.diagnostics[0];self.assertEqual('explicit-contract',d['reason']['classification_source']);self.assertEqual(['planet'],d['reason']['evidence']['unproven_or_incompatible_candidates'])
 def test_reason_id_stable(self):
  c=self.c();c.gap('parameter subgrammar not specified: X');d=c.diagnostics[0];identity=d['diagnostic_id'];classify(d);self.assertEqual(identity,d['diagnostic_id'])
 def test_unknown_reason_rejected(self):
  with self.assertRaises(ValueError):classify({'key':'x','message':'x','severity':'warning'},'invented')
 def test_queue_native_chain(self):
  c=self.run_script('queue_actions={repeat={find_random_system={trigger={id=S has_star_flag=test} found_system={move_to=this}} find_closest_planet={trigger={id=P is_star=yes} found_planet={orbit_planet=this wait={duration=180 random=180}}}}}')
  self.assertFalse(c.diagnostics)
 def test_queue_move_country(self):self.assertTrue(self.errors(self.run_script('queue_actions={move_to=root}',scope='country')))
 def test_queue_orbit_wrong_object(self):self.assertTrue(self.errors(self.run_script('queue_actions={orbit_planet=this}')))
 def test_queue_trigger_wrong_scope(self):self.assertTrue(self.errors(self.run_script('queue_actions={find_random_system={trigger={id=X is_ai=no}}}')))
 def test_queue_missing_id(self):self.assertTrue(self.errors(self.run_script('queue_actions={effect={set_fleet_flag=ok}}')))
 def test_queue_duplicate_id(self):self.assertTrue(self.errors(self.run_script('queue_actions={effect={id=X set_fleet_flag=ok} repeat={while={id=X always=yes}}}')))
 def test_queue_effect_scope(self):self.assertTrue(self.errors(self.run_script('queue_actions={effect={id=X set_country_flag=bad}}')))
 def test_queue_effect_valid(self):self.assertFalse(self.run_script('queue_actions={effect={id=X set_fleet_flag=ok}}').diagnostics)
 def test_queue_bare_effect_not_silently_accepted(self):self.assertTrue(self.errors(self.run_script('queue_actions={set_fleet_flag=bad}')))
 def test_queue_unknown_action_stays_gap(self):
  c=self.run_script('queue_actions={unknown_action={always=yes}}');self.assertFalse(self.errors(c));self.assertTrue(c.gaps);self.assertEqual('rule-unimplemented',c.diagnostics[0]['reason']['code'])
 def test_queue_wait_bad_field(self):self.assertTrue(self.errors(self.run_script('queue_actions={wait={duration=10 radnom=1}}')))
 def test_queue_wait_number(self):self.assertTrue(self.errors(self.run_script('queue_actions={wait={duration="!"}}')))
 def test_queue_found_wrong_field(self):self.assertTrue(self.errors(self.run_script('queue_actions={find_closest_planet={found_system={move_to=this}}}')))
 def test_queue_repeat_boolean_shape(self):self.assertTrue(self.errors(self.run_script('queue_actions={repeat=yes}')))
 def test_start_situation_callback_scope(self):
  c=self.run_script('start_situation={type=known effect={save_global_event_target_as=test}}','country');self.assertEqual('situation',c.global_targets['global_event_target:test']);self.assertFalse(self.errors(c))
 def test_start_situation_bad_callback(self):self.assertTrue(self.errors(self.run_script('start_situation={type=known effect={set_country_flag=bad}}','country')))
 def test_arrival_named_effect(self):
  c=self.c();c.scripts['effects']['arrival']=(ast('arrival={set_fleet_flag=ok}')[0],'effect.txt');c.script(ast('auto_move_to_planet={target=event_target:dest arrival_effect=arrival}'),'effects',Context('fleet','country'));self.assertFalse(c.diagnostics);self.assertIn(('effects','arrival'),c.invoked)
 def test_arrival_bad_callback(self):
  c=self.c();c.scripts['effects']['arrival']=(ast('arrival={set_country_flag=bad}')[0],'effect.txt');c.script(ast('auto_move_to_planet={arrival_effect=arrival}'),'effects',Context('fleet','country'));self.assertTrue(self.errors(c))
 def test_arrival_missing_callback(self):self.assertTrue(self.errors(self.run_script('auto_move_to_planet={arrival_effect=missing}')))
 def test_arrival_ship_not_assumed(self):
  c=self.run_script('auto_move_to_planet={target=event_target:dest arrival_effect={set_fleet_flag=ok}}','ship');self.assertTrue(c.gaps);self.assertFalse(self.errors(c))
 def test_country_ethos_valid(self):
  c=self.c();c.entities['ethics'].add('ethic_test');c.script(ast('create_country={ethos={ethic=ethic_test}}'),'effects',Context('country','country'));self.assertFalse(c.diagnostics)
 def test_country_ethos_unknown_ref(self):self.assertTrue(self.errors(self.run_script('create_country={ethos={ethic=missing}}','country')))
 def test_country_ethos_bad_field(self):self.assertTrue(self.errors(self.run_script('create_country={ethos={ethiic=random}}','country')))
 def test_fleet_garrison_bool(self):self.assertTrue(self.errors(self.run_script('set_fleet_settings={garrison=maybe}')))
 def test_onaction_override_scope(self):self.assertTrue(self.errors(self.run_script('fire_on_action={on_action=test scopes={from=this.wrong}}')))
 def test_onaction_override_typo(self):self.assertTrue(self.errors(self.run_script('fire_on_action={on_action=test scopes={form=this}}')))
 def test_onaction_missing_name(self):self.assertTrue(self.errors(self.run_script('fire_on_action={scopes={from=this}}')))
 def test_parameters_inline_really_checked(self):
  with tempfile.TemporaryDirectory() as tmp:
   p=Path(tmp)/'p.txt';p.write_text('add_trait=missing');c=self.c();c.inline['traits']=p;c.script(ast('modify_species={inline_script=traits}'),'effects',Context('species','species'));self.assertTrue(self.errors(c));self.assertFalse(any('inline_script' in d['message'] and d['key']=='validation-coverage' for d in c.diagnostics))
 def test_old_report_all_reasons_defined(self):
  r=json.loads((ROOT/'tests/fixtures/diagnostic-reasons-016.json').read_text(encoding='utf8'));rows=summary(r['diagnostics']);self.assertEqual(len(r['diagnostics']),sum(g['count'] for g in rows));self.assertFalse(any(g['code']=='unclassified-review' for g in rows))
 def test_situation_missing_type(self):self.assertTrue(self.errors(self.run_script('start_situation={effect={save_event_target_as=x}}','country')))
 def test_situation_missing_reference(self):self.assertTrue(self.errors(self.run_script('start_situation={type=missing}','country')))
 def test_arrival_target_wrong_type(self):self.assertTrue(self.errors(self.run_script('auto_move_to_planet={target=this}')))
 def test_arrival_target_missing(self):self.assertTrue(self.errors(self.run_script('auto_move_to_planet={clear_auto_move_on_arrival=yes}')))
 def test_arrival_flag_not_boolean(self):self.assertTrue(self.errors(self.run_script('auto_move_to_planet={target=event_target:dest clear_auto_move_on_arrival=maybe}')))
if __name__=='__main__':unittest.main()
