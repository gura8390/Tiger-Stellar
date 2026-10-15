import json, re, sys, tempfile, unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'src'))
from native_parser import parse_text, parse_many
from stellar import Checker, Context, children, load
from templates import render
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))

class Regressions(unittest.TestCase):
    def check(self,text,scope='country',kind='triggers',froms=()):
        parsed=parse_text('test = { '+text+' }');self.assertEqual([],parsed['diagnostics'])
        c=Checker(RULES);c.file='fixture.txt';c.script(children(parsed['ast'][0]),kind,Context(scope,scope,froms));return c
    def errors(self,c):return [d for d in c.diagnostics if d['severity'] in {'error','fatal'}]
    def test_positive_scope_chains(self):
        cases=[('is_ai = no','country'),('owner = { is_ai = no }','planet'),('capital_scope.owner = { is_ai = no }','country'),('root.capital_scope = { owner = { is_ai = no } }','country'),('any_owned_planet = { owner = { is_ai = no } }','country'),('any_system_planet = { owner = { is_ai = no } }','galactic_object'),('any_owned_colony = { owner = { is_ai = no } }','country'),('capital_scope = { prev = { is_ai = no } }','country')]
        for text,scope in cases:
            with self.subTest(text=text):self.assertEqual([],self.errors(self.check(text,scope)))
    def test_wrong_input_scope(self):
        for text,scope in [('is_ai = no','planet'),('capital_scope = { always = yes }','species'),('any_owned_planet = { always = yes }','species'),('any_system_planet = { always = yes }','country')]:
            with self.subTest(text=text):self.assertTrue(any(d['key']=='scope' for d in self.errors(self.check(text,scope))))
    def test_wrong_output_scope(self):
        for text in ['capital_scope = { is_ai = no }','any_owned_planet = { is_ai = no }','capital_scope.owner = { is_star = yes }']:
            with self.subTest(text=text):self.assertTrue(any(d['key']=='scope' for d in self.errors(self.check(text))))
    def test_prev_without_scope(self):self.assertTrue(self.errors(self.check('prev = { always = yes }')))
    def test_unknown_scope(self):self.assertTrue(self.errors(self.check('capital_scope.misspelled_owner = { is_ai = no }')))
    def test_from_scope(self):self.assertEqual([],self.errors(self.check('from = { is_ai = no }','galactic_object',froms=('country',))))
    def test_wrong_from_scope(self):self.assertTrue(self.errors(self.check('from = { is_ai = no }','galactic_object',froms=('planet',))))
    def test_unknown_from_is_warning(self):
        c=self.check('from = { is_ai = no }');self.assertFalse(self.errors(c));self.assertTrue(c.gaps)
    def test_unknown_trigger(self):self.assertTrue(any(d['key']=='unknown-trigger' for d in self.errors(self.check('is_aai = no'))))
    def test_unknown_effect(self):self.assertTrue(any(d['key']=='unknown-effect' for d in self.errors(self.check('set_countri_flag = x',kind='effects'))))
    def test_boolean(self):self.assertTrue(any(d['key']=='value' for d in self.errors(self.check('is_ai = potato'))))
    def test_scalar_block(self):self.assertTrue(any(d['key']=='structure' for d in self.errors(self.check('is_ai = { always = yes }'))))
    def test_block_scalar(self):self.assertTrue(any(d['key']=='structure' for d in self.errors(self.check('set_variable = yes',kind='effects'))))
    def test_effect_scope(self):self.assertTrue(any(d['key']=='scope' for d in self.errors(self.check('set_owner = root',kind='effects'))))
    def test_set_owner_target(self):
        self.assertFalse(self.errors(self.check('set_owner = owner','planet','effects')))
        self.assertTrue(self.errors(self.check('set_owner = this','planet','effects')))
    def test_iterator_effect(self):
        self.assertFalse(self.errors(self.check('every_owned_planet = { owner = { set_country_flag = test } }',kind='effects')))
        self.assertTrue(self.errors(self.check('every_owned_planet = { set_country_flag = test }',kind='effects')))
    def test_fleet_iterator_output(self):
        self.assertFalse(self.errors(self.check('every_fleet_in_system = { clear_orders = yes }','galactic_object','effects')))
        self.assertTrue(self.errors(self.check('every_fleet_in_system = { set_country_flag = test }','galactic_object','effects')))
    def test_pop_group_iterators_keep_group_type(self):
        for kind,name in [('triggers','any_owned_pop_group'),('triggers','any_species_pop_group'),('effects','every_owned_pop_group'),('effects','random_owned_pop_group')]:
            scope='species' if 'species' in name else 'country'
            good='owner = { is_ai = no }' if kind=='triggers' else 'owner = { set_country_flag = group_owner }'
            bad='is_ai = no' if kind=='triggers' else 'set_country_flag = group_owner'
            with self.subTest(name=name):
                self.assertFalse(self.errors(self.check(name+' = { '+good+' }',scope,kind)))
                self.assertTrue(self.errors(self.check(name+' = { '+bad+' }',scope,kind)))
    def test_country_iterators_named_by_location(self):
        for name,scope in [('any_country_neighbor_to_system','galactic_object'),('any_federation_ally','country')]:
            with self.subTest(name=name):
                self.assertFalse(self.errors(self.check(name+'={ is_ai=no }',scope)))
                self.assertTrue(self.errors(self.check(name+'={ is_star=yes }',scope)))
    def test_parser_cache_does_not_share_mutable_ast(self):
        from unittest.mock import patch
        from native_parser import _parse_text_payload
        import subprocess
        _parse_text_payload.cache_clear()
        with patch('native_parser.subprocess.run',wraps=subprocess.run) as call:
            first=parse_text('cache_unique={always=yes}')
            first['ast'][0]['children'][0]['line']=999
            second=parse_text('cache_unique={always=yes}')
            self.assertEqual(1,call.call_count)
            self.assertNotEqual(999,second['ast'][0]['children'][0]['line'])
    def test_unknown_parameter(self):self.assertTrue(any(d['key']=='unknown-field' for d in self.errors(self.check('set_variable = { which = x vaule = 1 }',kind='effects'))))
    def test_missing_parameter(self):self.assertTrue(any(d['key']=='missing-field' for d in self.errors(self.check('set_variable = { which = x }',kind='effects'))))
    def test_valid_parameters(self):self.assertFalse(self.errors(self.check('set_variable = { which = x value = 1 }',kind='effects')))
    def test_technology_reference(self):
        self.assertTrue(any(d['key']=='missing-item' for d in self.errors(self.check('has_technology = tech_does_not_exist'))))
        self.assertTrue(self.errors(self.check('has_technology = root_typo')))
    def test_technology_positive(self):
        c=Checker(RULES);c.entities['technology'].add('tech_ok');n=parse_text('has_technology = tech_ok')['ast'];c.script(n,'triggers',Context('country','country'));self.assertFalse(self.errors(c))
    def test_modifiers(self):
        for name,expected in [('planet_housing_add',False),('planet_houssing_add',True)]:
            c=Checker(RULES);c.modifier(parse_text('x={ '+name+'=2 }')['ast'][0],Context('planet','planet'));self.assertEqual(expected,bool(self.errors(c)))
    def test_native_syntax(self):
        for text in ['x={ always=yes','x={ a = }','x = }','x={ a = "unterminated }']:
            with self.subTest(text=text):self.assertTrue(parse_text(text)['diagnostics'])
    def test_multiline_and_long_strings(self):self.assertFalse(parse_text('x = "'+('中'*300)+'\ntext"')['diagnostics'])
    def test_stellaris_optional_scope_and_qualified_identifiers(self):
        for text in ['x = { owner? = { always = yes } }','x = { has_country_flag = example@fromfrom }','x = { id = example^suffix }']:
            with self.subTest(text=text):self.assertFalse(parse_text(text)['diagnostics'])
        self.assertTrue(parse_text('x = { owner? = }')['diagnostics'])
    def test_global_local_shadow(self):
        result=parse_text('@cost=2\nx={ value=@cost }',globals_text='@cost=1');self.assertFalse(result['diagnostics']);self.assertEqual('2',children(result['ast'][0])[0]['value'])
    def test_undefined_variable(self):self.assertTrue(any(d['key']=='reader-directives' for d in parse_text('x={ value=@absent }')['diagnostics']))
    def test_duplicate_local_variable(self):self.assertTrue(parse_text('@cost=1\n@cost=2\nx={v=@cost}')['diagnostics'])
    def test_reader_expression(self):
        p=parse_text('x = { v = @\\[( 72 * $PROGRESS$ )] }');self.assertFalse(p['diagnostics']);self.assertIn('PROGRESS',p['ast'][0]['macro_parameters'])
        self.assertTrue(parse_text('x = { v = @\\[( 72 * 0.2 ) }')['diagnostics'])
    def test_optional_template(self):
        text='[[A] is_ai = yes ] [[!A] is_ai = no ]'
        self.assertIn('is_ai = yes',render(text,{'A':'x'}));self.assertNotIn('is_ai = no',render(text,{'A':'x'}))
        self.assertIn('is_ai = no',render(text,{}));self.assertNotIn('is_ai = yes',render(text,{}))
    def test_nested_template(self):self.assertIn('value=2',render('[[A] [[B] value=$V|2$ ] ]',{'A':'1','B':'1'}))
    def test_template_excluded_missing(self):self.assertNotIn('value',render('[[A] value=$MISSING$ ]',{}))
    def test_template_missing(self):
        with self.assertRaises(ValueError):render('value=$MISSING$',{})
    def test_template_default(self):self.assertEqual('value=no',render('value=$VALUE|no$',{}))
    def test_template_comments(self):self.assertIn('# $NOT_AN_ARG$',render('# $NOT_AN_ARG$\nv=1',{}))
    def test_native_conditional(self):
        self.assertFalse(parse_text('x={ [[A] always=yes ] }')['diagnostics']);self.assertTrue(parse_text('x={ [[A] always=yes }')['diagnostics'])
    def test_scripted_macro(self):
        c=Checker(RULES);c.scripts['triggers']['custom_test']=(parse_text('custom_test={ $SCOPE$ = { is_ai=$VALUE|no$ } }')['ast'][0],'definition.txt')
        c.script(parse_text('custom_test={ SCOPE=owner }')['ast'],'triggers',Context('planet','planet'));self.assertFalse(self.errors(c))
        c.script(parse_text('custom_test={ SCOPE=planet }')['ast'],'triggers',Context('country','country'));self.assertTrue(self.errors(c))
    def test_scripted_macro_required(self):
        c=Checker(RULES);c.scripts['triggers']['custom_test']=(parse_text('custom_test={ is_ai=$VALUE$ }')['ast'][0],'definition.txt');c.script(parse_text('custom_test=yes')['ast'],'triggers',Context('country','country'));self.assertTrue(any(d['key']=='macro-argument' for d in self.errors(c)))
    def test_scripted_optional_absent_parameter(self):
        c=Checker(RULES);c.scripts['triggers']['custom_test']=(parse_text('custom_test={ [[A] is_ai=$VALUE$ ] [[!A] always=yes ] }')['ast'][0],'definition.txt');c.script(parse_text('custom_test=yes')['ast'],'triggers',Context('country','country'));self.assertFalse(self.errors(c))
    def test_all_documented_scope_links(self):
        counts={'positive':0,'wrong_input':0,'wrong_output':0}
        for name,rule in RULES['scopes'].items():
            if name in {'this','root','prev','from','no_scope'} or re.fullmatch(r'(?:prev|from)+',name) or rule['output']=='various':continue
            accepted=rule['input'];scope='country' if 'all' in accepted else accepted[0]
            ctx=Context(scope,scope,('country',)*4,(scope,)*4)
            c=Checker(RULES);n={'key':name};resolved=c.resolve(name,ctx,n);self.assertFalse(self.errors(c),name);self.assertEqual(rule['output'],resolved.current,name);counts['positive']+=1
            c.rules['triggers']['__probe']={'input':[rule['output']],'description':'probe'}
            c.script([{'key':'__probe','value':'yes'}],'triggers',resolved);self.assertFalse(self.errors(c),name)
            c.rules['triggers']['__bad_probe']={'input':['__invalid_scope'],'description':'probe'};c.script([{'key':'__bad_probe','value':'yes'}],'triggers',resolved);self.assertTrue(self.errors(c),name);counts['wrong_output']+=1
            if 'all' not in accepted:
                c=Checker(RULES);c.resolve(name,Context('__invalid_scope','__invalid_scope'),n);self.assertTrue(self.errors(c),name);counts['wrong_input']+=1
        (ROOT/'verification/scope-matrix.json').write_text(json.dumps(counts,indent=2),encoding='utf8')
    def test_scripted_context(self):
        for scope,expected in [('country',False),('planet',True)]:
            c=Checker(RULES);c.scripts['triggers']['custom_test']=(parse_text('custom_test={ is_ai=no }')['ast'][0],'definition.txt');c.script(parse_text('custom_test=yes')['ast'],'triggers',Context(scope,scope));self.assertEqual(expected,bool(self.errors(c)))
    def test_resource_reference(self):self.assertTrue(any(d['key']=='missing-item' for d in self.errors(self.check('resource_stockpile_compare = { resource = missing_resource value > 1 }'))))
    def test_event_reference(self):self.assertTrue(any(d['key']=='missing-event' for d in self.errors(self.check('country_event = { id = missing.1 }',kind='effects'))))
    def test_unknown_parameter_coverage(self):
        c=self.check('create_country = { nonexistent_parameter = test }',kind='effects');self.assertTrue(c.gaps)
    def test_overlay_replace_path_and_globals(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);game=root/'game';mod=root/'mod'
            def put(base,rel,text):p=base/rel;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf8')
            put(game,'common/technology/a.txt','removed_tech={}');put(game,'common/technology/b.txt','also_removed={}');put(game,'common/scripted_variables/a.txt','@cost=1')
            put(mod,'descriptor.mod','replace_path="common/technology"');put(mod,'common/technology/a.txt','new_tech={}');put(mod,'common/buildings/test.txt','test={ base_buildtime=@cost }')
            c=Checker(RULES);target=load(c,game,[mod]);self.assertEqual({'new_tech'},c.entities['technology']);self.assertFalse(self.errors(c));self.assertEqual(2,len(target))

if __name__=='__main__':unittest.main(verbosity=2)

