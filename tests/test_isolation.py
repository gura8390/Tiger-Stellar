"""Cross-game, wrong installation, provenance and independent-build regressions."""
import copy, json, sys, tempfile, unittest, argparse
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'src'))
from stellar import Checker, Context, run
from native_parser import parse_text
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))

class Isolation(unittest.TestCase):
    def test_foreign_game_tokens_rejected(self):
        cases=[('clerical_region_title = { always=yes }','triggers'),
               ('any_county_in_clerical_region = { always=yes }','triggers'),
               ('faith = { always=yes }','triggers'),
               ('add_piety = 10','effects'),('add_prestige = 10','effects'),
               ('government_has_flag = government_is_feudal','triggers'),
               ('add_political_power = 10','effects'),
               ('has_war_support > 0.5','triggers')]
        for text,kind in cases:
            with self.subTest(text=text):
                c=Checker(RULES);c.script(parse_text(text)['ast'],kind,Context('country','country'))
                self.assertTrue(any(d['severity']=='error' and d['key'] in {'unknown-trigger','unknown-effect','unknown-scope'} for d in c.diagnostics),c.diagnostics)
    def test_foreign_rules_rejected(self):
        for game in ['ck3','hoi4','vic3',None]:
            rules=copy.deepcopy(RULES);rules['game_id']=game
            with self.subTest(game=game), self.assertRaisesRegex(ValueError,'wrong-game'):Checker(rules)
    def test_foreign_declaration_rules_rejected(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/'rules').mkdir()
            (root/'rules/declarations-4.5.2.json').write_text(json.dumps({'game_id':'ck3','categories':{}}),encoding='utf8')
            with patch('stellar.resource_root',return_value=root),self.assertRaisesRegex(ValueError,'wrong-game'):Checker(RULES)
    def test_foreign_scope_cannot_enter_stellaris_marked_rules(self):
        for foreign in ['character','landed_title','faith','market','state']:
            rules=copy.deepcopy(RULES);rules['scopes']['foreign_scope']={'input':['country'],'output':foreign}
            with self.subTest(scope=foreign),self.assertRaisesRegex(ValueError,'non-Stellaris scope'):Checker(rules)
    def test_shared_name_uses_stellaris_definition(self):
        c=Checker(RULES);c.entities['governments'].add('gov_despotic_empire')
        c.script(parse_text('has_government=gov_despotic_empire')['ast'],'triggers',Context('country','country'))
        self.assertFalse(c.diagnostics)
        c.script(parse_text('has_government=theocracy_government')['ast'],'triggers',Context('country','country'))
        self.assertTrue(any(d['key']=='missing-item' and d['severity']=='error' for d in c.diagnostics))
    def test_foreign_installation_rejected(self):
        for game in ['ck3','hoi4','vic3',None]:
            with tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp)
                if game:(root/'launcher-settings.json').write_text(json.dumps({'gameId':game,'rawVersion':'v4.5.2'}),encoding='utf8')
                args=argparse.Namespace(game=root,rules=ROOT/'rules/stellaris-4.5.2.json')
                with self.subTest(game=game),self.assertRaisesRegex(ValueError,'wrong-game'):run(args)
    def test_no_foreign_backend_dependency(self):
        cargo=(ROOT/'upstream/stellaris-parser/Cargo.toml').read_text(encoding='utf8')
        for foreign in ['tiger-lib','tiger-tables','ck3','hoi4','vic3','imperator','eu5']:self.assertNotIn(foreign,cargo)
        self.assertIn('stellaris_parser = []',cargo)
        lib=(ROOT/'upstream/stellaris-parser/src/lib.rs').read_text(encoding='utf8')
        for foreign in ['mod ck3','mod hoi4','mod everything','mod scopes','mod trigger','mod effect']:self.assertNotIn(foreign,lib)
    def test_rules_only_stellaris_scopes(self):
        foreign={'character','landed_title','county','faith','religion','province','state','market','culture','combat','front'}
        for kind in ['triggers','effects','scopes']:
            for name,rule in RULES[kind].items():
                self.assertFalse(set(rule['input'])&foreign,(kind,name,rule['input']))
                self.assertFalse(set(rule.get('output','').split('|'))&foreign,(kind,name))
    def test_missing_semantic_coverage_remains_visible(self):
        c=Checker(RULES);c.script(parse_text('create_country={ unimplemented_parameter=yes }')['ast'],'effects',Context('country','country'))
        self.assertTrue(any(d['severity']=='warning' and d['key']=='validation-coverage' for d in c.diagnostics))

if __name__=='__main__':unittest.main(verbosity=2)
