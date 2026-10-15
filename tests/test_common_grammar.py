import json,sys,tempfile,unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'src'))
from stellar import Checker,Context,load
from native_parser import parse_text
RULES=json.loads((ROOT/'rules/stellaris-4.5.2.json').read_text(encoding='utf8'))
def errors(c):return [d for d in c.diagnostics if d['severity'] in {'error','fatal'}]
def node(text):return parse_text(text)['ast'][0]

class CommonGrammar(unittest.TestCase):
    def test_release_subject_null_target_is_specific(self):
        c=Checker(RULES);ctx=Context('country','country')
        c.script(parse_text('every_subject={set_subject_of={who=none}}')['ast'],'effects',ctx);self.assertFalse(c.diagnostics)
        c.script(parse_text('set_subject_of={who=none allow_instant_negotiation=potato} set_subject_of={who=capital_scope}')['ast'],'effects',ctx)
        self.assertTrue(any(d['key']=='value' for d in errors(c)));self.assertTrue(any(d['key']=='scope' for d in errors(c)))
        c.script(parse_text('capital_scope={set_subject_of={who=none}}')['ast'],'effects',ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_species_iterators_without_iterate_prose(self):
        c=Checker(RULES);ctx=Context('country','country')
        for name in ['every_owned_species','random_owned_species','ordered_owned_species','every_enslaved_species','ordered_galaxy_species']:
            c.script(parse_text(name+'={limit={is_archetype=MACHINE} set_species_flag=ok}')['ast'],'effects',ctx)
        self.assertFalse(errors(c))
        c.script(parse_text('every_owned_species={set_country_flag=bad}')['ast'],'effects',ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_spynetwork_target_country_is_documented(self):
        c=Checker(RULES);ctx=Context('country','country')
        c.script(parse_text('any_spynetwork={target={is_ai=no}}')['ast'],'triggers',ctx);self.assertFalse(c.diagnostics)
        c.script(parse_text('any_spynetwork={target={is_star=no}}')['ast'],'triggers',ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
        rules=json.loads(json.dumps(RULES));rules['scopes']['target']['output_by_input']={'spy_network':'character'}
        with self.assertRaisesRegex(ValueError,'wrong-game'):Checker(rules)
    def test_engine_documented_iterator_outputs(self):
        expected={'spynetwork':'spy_network','owned_contract':'mission','envoy':'leader','exhibit':'exhibit','available_debris':'debris','orbital_station':'ship','cosmic_storm':'cosmic_storm','system_within_storm':'galactic_object','subject':'country','owned_design':'design','moon':'planet','existing_species_traits':'trait','starbase_in_network':'starbase'}
        for stem,scope in expected.items():
            for kind,prefix in [('triggers','any_'),('effects','random_')]:
                name=prefix+stem
                if name not in RULES[kind]:continue
                with self.subTest(name=name):
                    c=Checker(RULES);c.rules[kind]['fixture_scope_probe']={'input':[scope],'boolean':True,'description':'fixture_scope_probe = yes'}
                    ctx=Context(RULES[kind][name]['input'][0],RULES[kind][name]['input'][0])
                    c.script(parse_text(name+'={fixture_scope_probe=yes}')['ast'],kind,ctx);self.assertFalse(errors(c))
                    c.rules[kind]['fixture_scope_probe']['input']=['planet' if scope!='planet' else 'country']
                    c.script(parse_text(name+'={fixture_scope_probe=yes}')['ast'],kind,ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_all_documented_iterators_have_outputs(self):
        for kind in ['triggers','effects']:
            for name,rule in RULES[kind].items():
                if 'Iterate through' in rule['description'] or 'Iterates through' in rule['description']:
                    with self.subTest(name=name):self.assertIn('iterator_output',rule)
    def test_order_expression_reference_and_type(self):
        ctx=Context('planet','planet');c=Checker(RULES)
        c.order_expression(node('order_by=trigger:distance_to_capital'),ctx);self.assertFalse(c.diagnostics)
        c.order_expression(node('order_by=trigger:distance_to_capitol'),ctx);self.assertTrue(any(d['key']=='unknown-trigger' for d in errors(c)))
        c.order_expression(node('order_by=trigger:is_ai'),Context('country','country'));self.assertTrue(any(d['key']=='value' for d in errors(c)))
        c.order_expression(node('order_by=trigger:distance_to_capital'),Context('species','species'));self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_foreign_iterator_type_rejected(self):
        rules=json.loads(json.dumps(RULES));rules['triggers']['foreign_iterator']={'input':['country'],'iterator_output':'character'}
        with self.assertRaisesRegex(ValueError,'wrong-game'):Checker(rules)
    def test_ordered_iterator_metadata_values(self):
        c=Checker(RULES);ctx=Context('country','country')
        c.script(parse_text('ordered_owned_planet={position=0 order_by=trigger:distance_to_capital inverse=yes set_planet_flag=ok}')['ast'],'effects',ctx);self.assertFalse(errors(c))
        c.script(parse_text('ordered_owned_planet={position="!" inverse=potato set_country_flag=bad}')['ast'],'effects',ctx)
        self.assertEqual(2,sum(d['key']=='value' for d in errors(c)));self.assertTrue(any(d['key']=='scope' for d in errors(c)))
        c.script(parse_text('every_owned_planet={inverse=yes}')['ast'],'effects',ctx);self.assertTrue(any(d['key']=='unknown-effect' for d in errors(c)))
    def test_weight_clamps_and_complex_potential(self):
        c=Checker(RULES);ctx=Context('leader','leader')
        c.weighted(node('x={min=1 max=10 set=2 complex_trigger_modifier={trigger=has_base_skill potential={has_base_skill>0} mode=subtract mult=5}}'),'triggers',ctx);self.assertFalse(errors(c))
        c.weighted(node('x={min="!" complex_trigger_modifier={trigger=has_base_skill potential={is_star=yes} mode=add}}'),'triggers',ctx)
        self.assertTrue(any(d['key']=='value' for d in errors(c)));self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_run_in_ai_mode_only_in_event_effect_metadata(self):
        c=Checker(RULES);c.event(node('country_event={id=test.1 immediate={run_in_ai_mode=yes set_country_flag=ok}}'));self.assertFalse(errors(c))
        c.event(node('country_event={id=test.2 immediate={run_in_ai_mode=potato}}'));self.assertTrue(any(d['key']=='value' for d in errors(c)))
        c.script(parse_text('run_in_ai_mode=yes')['ast'],'effects',Context('country','country'));self.assertTrue(any(d['key']=='unknown-effect' for d in errors(c)))
    def test_custom_gui_requires_existing_container(self):
        c=Checker(RULES);c.entities['gui_containers'].add('enclave_caravaneer_option')
        ctx=Context('country','country')
        c.script(parse_text('custom_gui="enclave_caravaneer_option"')['ast'],'effects',ctx);self.assertFalse(errors(c))
        c.script(parse_text('custom_gui="enclave_caravaneer_optoin"')['ast'],'effects',ctx)
        self.assertTrue(any(d['key']=='missing-item' for d in errors(c)))
        c.script(parse_text('custom_gui={always=yes}')['ast'],'effects',ctx)
        self.assertTrue(any(d['key']=='structure' for d in errors(c)))
    def test_gui_registry_respects_mod_file_override(self):
        with tempfile.TemporaryDirectory() as tmp:
            game=Path(tmp)/'game';mod=Path(tmp)/'mod'
            for base,name in [(game,'old_container'),(mod,'new_container')]:
                p=base/'interface/test.gui';p.parent.mkdir(parents=True);p.write_text('guiTypes={containerWindowType={name='+name+'}}',encoding='utf8')
            c=Checker(RULES);load(c,game,[mod])
            self.assertEqual({'new_container'},c.entities['gui_containers'])
    def test_complex_trigger_scope_changes_context(self):
        ctx=Context('country','country');c=Checker(RULES)
        c.weighted(node('x={complex_trigger_modifier={trigger=has_base_skill trigger_scope=leader mode=add mult=5}}'),'triggers',ctx)
        self.assertFalse(errors(c))
        c.weighted(node('x={complex_trigger_modifier={trigger=has_base_skill trigger_scope=this mode=add mult=5}}'),'triggers',ctx)
        self.assertTrue(any(d['key']=='scope' for d in errors(c)))
        c.weighted(node('x={complex_trigger_modifier={trigger=has_base_skill trigger_scope=capitol_scope mode=add}}'),'triggers',ctx)
        self.assertTrue(any(d['key']=='unknown-scope' for d in errors(c)))
    def test_description_conditionals_validate_limit(self):
        c=Checker(RULES);c.loc.add('text_ok');ctx=Context('country','country')
        c.description(node('desc={if={limit={is_ai=no} text=text_ok} else_if={limit={always=yes} text=text_ok} else={text=text_ok}}'),ctx)
        self.assertFalse(errors(c))
        c.description(node('desc={if={limit={is_star=yes} text=text_ok} else={txet=text_ok}}'),ctx)
        self.assertTrue(any(d['key']=='scope' for d in errors(c)));self.assertTrue(any(d['key']=='unknown-field' for d in errors(c)))
        c.description(node('desc={if={text=text_ok}}'),ctx);self.assertTrue(any(d['key']=='missing-field' for d in errors(c)))
    def test_psionic_aura_boolean_and_reference(self):
        c=Checker(RULES);c.entities['patrons/psionic_auras'].add('end_aura');ctx=Context('galactic_object','galactic_object')
        c.script(parse_text('has_psionic_aura=end_aura has_psionic_aura=no')['ast'],'triggers',ctx);self.assertFalse(errors(c))
        c.script(parse_text('has_psionic_aura=end_auar')['ast'],'triggers',ctx);self.assertTrue(any(d['key']=='missing-item' for d in errors(c)))
        c.script(parse_text('has_psionic_aura=end_aura')['ast'],'triggers',Context('country','country'));self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_robotic_job_pretrigger_is_boolean(self):
        c=Checker(RULES);c.definition('pop_jobs',node('test={possible_pre_triggers={is_robotic=no}}'));self.assertFalse(errors(c))
        c.definition('pop_jobs',node('test={possible_pre_triggers={is_robotic=potato is_robotci=no}}'))
        self.assertTrue(any(d['key']=='value' for d in errors(c)));self.assertTrue(any(d['key']=='unknown-field' for d in errors(c)))
    def test_trait_iteration_outputs_trait(self):
        for name in ['any_trait_of_species','any_trait_available_for_species']:
            c=Checker(RULES);ctx=Context('species','species')
            c.script(parse_text(name+'={trait_has_any_tag={biological}}')['ast'],'triggers',ctx);self.assertFalse(errors(c))
            c.script(parse_text(name+'={is_ai=no}')['ast'],'triggers',ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_hidden_scope_prefix(self):
        c=Checker(RULES);ctx=Context('planet','planet')
        c.script(parse_text('hidden:owner={is_ai=no}')['ast'],'triggers',ctx);self.assertFalse(errors(c))
        c.script(parse_text('hidden:this={is_ai=no}')['ast'],'triggers',ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_weighted_inline_and_complex_trigger(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'weight.txt';p.write_text('modifier={factor=2 is_ai=no}',encoding='utf8')
            c=Checker(RULES);c.inline['weight']=p
            c.weighted(node('x={inline_script=WEIGHT complex_trigger_modifier={trigger=count_owned_planet parameters={limit={owner={is_ai=no}}} mode=add mult=2}}'),'triggers',Context('country','country'));self.assertFalse(errors(c))
            c.weighted(node('x={complex_trigger_modifier={trigger=count_owned_planet parameters={limit={is_ai=no}} mode=bogus}}'),'triggers',Context('country','country'));self.assertTrue(any(d['key']=='scope' for d in errors(c)));self.assertTrue(any(d['key']=='value' for d in errors(c)))
    def test_switch_case_inline(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'cases.txt';p.write_text('$FLAG$={set_country_flag=case_ok}',encoding='utf8')
            c=Checker(RULES);c.inline['cases']=p
            c.script(parse_text('switch={trigger=has_country_flag inline_script={script=cases FLAG=test}}')['ast'],'effects',Context('country','country'));self.assertFalse(errors(c))
            c.script(parse_text('switch={trigger=has_countri_flag test={set_country_flag=x}}')['ast'],'effects',Context('country','country'));self.assertTrue(any(d['key']=='unknown-trigger' for d in errors(c)))
    def test_logistics_and_overloaded_building_trigger(self):
        c=Checker(RULES);c.entities['strategic_resources'].add('trade');c.entities['districts'].add('district_mining')
        c.resources(node('x={logistics={trade=2}}'),Context('ship','ship'));self.assertFalse(errors(c))
        c.script(parse_text('has_district=district_mining')['ast'],'triggers',Context('planet','planet'));self.assertFalse(errors(c))
        c.script(parse_text('has_district=district_minning')['ast'],'triggers',Context('planet','planet'));self.assertTrue(any(d['key']=='missing-item' for d in errors(c)))
    def test_habitability_target_types(self):
        c=Checker(RULES);ctx=Context('planet','species')
        c.script(parse_text('habitability={who=root value>0.5}')['ast'],'triggers',ctx);self.assertFalse(errors(c))
        c.script(parse_text('habitability={who=this value>0.5}')['ast'],'triggers',ctx);self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_modifier_metadata_types(self):
        c=Checker(RULES);c.loc.add('tooltip_ok')
        c.modifier(node('x={icon=test icon_frame=2 custom_tooltip=tooltip_ok show_only_custom_tooltip=yes planet_housing_add=1}'),Context('planet','planet'))
        self.assertFalse(errors(c))
        c.modifier(node('x={show_only_custom_tooltip=potato}'),Context('planet','planet'))
        self.assertTrue(any(d['key']=='value' for d in errors(c)))
    def test_triggered_modifier_nested_body(self):
        for modifier,valid in [('planet_housing_add',True),('planet_houssing_add',False)]:
            c=Checker(RULES);c.modifier(node('x={potential={is_ai=no} modifier={'+modifier+'=2}}'),Context('country','country'),True)
            self.assertEqual(valid,not errors(c))
    def test_resource_mult_and_inline(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'resource.txt';path.write_text('cost={ $RESOURCE$=$COST$ mult=2 }',encoding='utf8')
            c=Checker(RULES);c.entities['strategic_resources'].add('minerals');c.inline['resource']=path
            c.resources(node('x={inline_script={script=resource RESOURCE=minerals COST=10}}'),Context('planet','planet'))
            self.assertFalse(errors(c))
            c.resources(node('x={cost={minerals=2 mull=3}}'),Context('planet','planet'))
            self.assertTrue(any(d['key']=='missing-item' for d in errors(c)))
    def test_new_event_types_keep_typed_roots(self):
        for kind in ['astral_rift_event','first_contact_event','espionage_operation_event','agreement_event','bypass_event','pop_faction_event']:
            with self.subTest(kind=kind):
                c=Checker(RULES);c.event(node(kind+'={id=test.1 is_triggered_only=yes trigger={always=yes}}'));self.assertFalse(errors(c))
                c.event(node(kind+'={id=test.2 immediate={set_country_flag=bad}}'));self.assertTrue(any(d['key']=='scope' for d in errors(c)))
    def test_event_metadata_boolean_and_weight(self):
        c=Checker(RULES);c.event(node('country_event={id=test.1 trackable=yes is_advisor_event=yes weight_multiplier={factor=2 modifier={factor=0 is_ai=yes}}}'));self.assertFalse(errors(c))
        c.event(node('country_event={id=test.2 trackable=potato}'));self.assertTrue(any(d['key']=='value' for d in errors(c)))
    def test_event_inheritance_clear_and_missing_base(self):
        c=Checker(RULES);c.event_defs['base.1']=(node('country_event={id=base.1 trigger={is_ai=no}}'),'base.txt')
        c.event(node('planet_event={id=test.1 base=base.1 trigger_clear=yes trigger={is_star=no}}'));self.assertFalse(errors(c))
        c.event(node('planet_event={id=test.2 base=base.1}'));self.assertTrue(any(d['key']=='scope' and d['path']=='base.txt' for d in errors(c)))
        c.event(node('country_event={id=test.3 base=missing.1}'));self.assertTrue(any(d['key']=='missing-event' for d in errors(c)))
    def test_event_inheritance_cycle(self):
        c=Checker(RULES);c.event_defs['base.1']=(node('country_event={id=base.1 base=base.1}'),'base.txt')
        c.event(node('country_event={id=test.1 base=base.1}'));self.assertTrue(any(d['key']=='inheritance-cycle' for d in errors(c)))
    def test_description_switch(self):
        c=Checker(RULES);c.loc.add('text_ok')
        c.event(node('country_event={id=test.1 desc={trigger={switch={trigger=has_country_flag flag_a={text=text_ok} default={text=text_ok}}}}}'));self.assertFalse(errors(c))
        c.event(node('country_event={id=test.2 desc={trigger={switch={trigger=has_countri_flag flag_a={text=text_ok}}}}}'));self.assertTrue(any(d['key']=='unknown-trigger' for d in errors(c)))
    def test_parameterized_script_values(self):
        for expression,key in [('known|MODIFIER|planet_housing_add|',None),('known|COST|','value'),('absent|COST|1|','missing-item'),('known|MODIFIER|planet_houssing_add|','unknown-modifier')]:
            c=Checker(RULES);c.entities['script_values'].add('known');c.numeric({'key':'value','value':'value:'+expression})
            with self.subTest(expression=expression):self.assertEqual(key is None,not errors(c))
    def test_ship_creation_extended_parameters(self):
        c=Checker(RULES);ctx=Context('fleet','fleet')
        c.script(parse_text('create_ship={design=example upgradable=no create_colony=yes age=10 growth_stage=2 rarity=epic}')['ast'],'effects',ctx);self.assertFalse(errors(c))
        c.script(parse_text('create_ship={design=example upgradable=potato rarity=bogus}')['ast'],'effects',ctx);self.assertEqual(2,len(errors(c)))
    def test_root_event_inline_registers_ids(self):
        with tempfile.TemporaryDirectory() as tmp:
            game=Path(tmp)/'game';mod=Path(tmp)/'mod'
            for base,rel,text in [(game,'common/inline_scripts/root_events.txt','country_event = {\n id=template.1\n is_triggered_only=yes\n}'),(game,'events/root.txt','inline_script=root_events'),(mod,'events/mod.txt','country_event={id=test.1 immediate={country_event={id=template.1}}}')]:
                p=base/rel;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf8')
            c=Checker(RULES);target=load(c,game,[mod]);self.assertIn('template.1',c.events)
            self.assertFalse(c.diagnostics)
            self.assertFalse(any(d['key']=='brace-placement' for d in c.vanilla_parse_diagnostics))  # A top-level closing brace is not nested in a synthetic wrapper.
            for path,category,ast in target:
                c.file=path
                for n in ast:c.definition(category,n)
            self.assertFalse(errors(c))

if __name__=='__main__':unittest.main(verbosity=2)
