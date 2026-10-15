import copy,json,tempfile,unittest
from argparse import Namespace
from pathlib import Path
from test_coverage_011 import checker,ast,ROOT
from reporting import attribute,annotate
from stellar import run

class Feedback014(unittest.TestCase):
 def test_cross_file_owner_and_separate_native_positions(self):
  a=checker();b=checker()
  for c,path in [(a,'D:/mod/common/ship_sizes/test.txt'),(b,'D:/game/common/ship_sizes/test.txt')]:
   nodes=ast('hull_a={required_component_set=empty}\nhull_b={required_component_set=empty}')
   for n in nodes:c.definitions['ship_sizes'][n['key']]=dict(n,_source_path=path)
   c.ship_crosschecks([(path,'ship_sizes',nodes)])
  attribute(a,b,Path('D:/game'),[Path('D:/mod')])
  for d in a.diagnostics:
   self.assertIn(d['definition'],{'hull_a','hull_b'});self.assertEqual('vanilla-existing',d['attribution']['classification']);self.assertTrue(d['attribution']['statement_same_as_native']);self.assertEqual(1,len(d['attribution']['control_diagnostic_positions']))
 def test_native_match_cannot_be_overwritten_without_owner(self):
  a=checker();b=checker();n=ast('required_component_set=empty')[0]
  a.file='D:/mod/common/ship_sizes/t.txt';b.file='D:/game/common/ship_sizes/t.txt'
  a.emit('ship-compatibility','empty',n);b.emit('ship-compatibility','empty',n)
  attribute(a,b,Path('D:/game'),[Path('D:/mod')]);d=a.diagnostics[0];self.assertEqual('vanilla-existing',d['attribution']['classification']);self.assertTrue(d['attribution']['statement_same_as_native'])
 def test_missing_owner_is_not_evidence_mod_added_statement(self):
  a=checker();b=checker();a.file='D:/mod/t.txt';a.emit('ship-compatibility','empty',ast('required_component_set=empty')[0]);attribute(a,b,Path('D:/game'),[Path('D:/mod')]);self.assertEqual('unconfirmed',a.diagnostics[0]['attribution']['classification'])
 def test_absent_native_diagnostic_does_not_invent_script_caller(self):
  a=checker();b=checker();n=ast('civic={modifier={unknown_014=1}}')[0]
  a.file='D:/mod/common/governments/civics/t.txt';b.file='D:/game/common/governments/civics/t.txt';b.definitions['governments/civics']['civic']=dict(n,_source_path=b.file)
  a.definition('governments/civics',n);attribute(a,b,Path('D:/game'),[Path('D:/mod')]);self.assertEqual('unconfirmed',a.diagnostics[0]['attribution']['classification'])
 def test_dependency_native_control_keeps_nested_category_and_severity(self):
  with tempfile.TemporaryDirectory() as t:
   root=Path(t);game=root/'game';dep=root/'dep';mod=root/'mod';dep.mkdir()
   game.mkdir();(game/'launcher-settings.json').write_text(json.dumps({'gameId':'stellaris','rawVersion':'v4.5.2'}))
   for folder,prefix in [(game,''),(mod,'\n\n')]:
    p=folder/'common/governments/civics/t.txt';p.parent.mkdir(parents=True);p.write_text(prefix+'civic={modifier={unknown_014=1}}',encoding='utf8')
   out=root/'out.json';code=run(Namespace(game=game,mod=mod,dependency=[dep],cache=None,syntax_only=False,rules=ROOT/'rules/stellaris-4.5.2.json',output=out,native_control=True))
   r=json.loads(out.read_text(encoding='utf8'));self.assertEqual(1,code);self.assertEqual(1,r['errors']);self.assertEqual('error',r['diagnostics'][0]['severity']);self.assertEqual('vanilla-existing',r['diagnostics'][0]['attribution']['classification']);self.assertEqual(1,len([d for d in r['native_control']['diagnostics'] if d['severity']=='error']))
 def test_genuine_hull_change_still_errors_and_is_not_inherited(self):
  a=checker();b=checker()
  for c,path,text in [(a,'D:/mod/common/ship_sizes/t.txt','hull={required_component_set=typo}'),(b,'D:/game/common/ship_sizes/t.txt','hull={required_component_set=old}')]:
   ns=ast(text);c.definitions['ship_sizes']['hull']=dict(ns[0],_source_path=path);c.ship_crosschecks([(path,'ship_sizes',ns)])
  attribute(a,b,Path('D:/game'),[Path('D:/mod')]);d=a.diagnostics[0];self.assertEqual('error',d['severity']);self.assertEqual('mod-new-or-modified',d['attribution']['classification'])
if __name__=='__main__':unittest.main()
