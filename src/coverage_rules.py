"""Stellaris declaration and creation contracts backed by installed examples.

These contracts validate structure, values, references and nested scopes. They
do not treat an observed block as an arbitrary legal script or suppress gaps.
"""
from handoff_rules import HandoffRules
from ship_contracts import ShipContracts
from engine_entries import EngineEntries
from assets_names import AssetsNames
from context_flow import ContextFlow
from coverage_extensions import CoverageExtensions
from fleet_actions import FleetActions
from entry_lists import EntryLists

class CoverageRules(HandoffRules,ShipContracts,EngineEntries,AssetsNames,ContextFlow,CoverageExtensions,FleetActions,EntryLists):
    def building_icon_asset(self,value,n):
        """Building icons may name a surviving DDS without a building entity."""
        if not value or any(s in value for s in ['..','/','\\']):return False
        import hashlib
        rel='gfx/interface/icons/buildings/'+value+'.dds'
        for root in reversed(self.asset_roots):
            path=root/rel
            if path.is_file():
                if str(path) not in self.asset_inputs:self.asset_inputs[str(path)]={'relative_path':rel,'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
                return True
        return False

    def infer_script_inputs(self,node,kind):
        """Infer a contract for a simple uncalled script, never invent a caller."""
        from stellar import children
        from stellar_scopes import ENGINE_SCOPE_TYPES
        candidates=set(ENGINE_SCOPE_TYPES)-{'all','various','no_scope'}
        constrained=False
        def gather(items):
            nonlocal candidates,constrained
            for item in items:
                k=item.get('key','');low=k.lower()
                if low in {'and','or','not','nor','nand','hidden_trigger','hidden_effect','limit'}:
                    if not gather(children(item)):return False
                else:
                    if k=='exists':rule=self.rules['scopes'].get(item.get('value','').split('.')[0])
                    elif k in self.rules['scopes']:rule=self.rules['scopes'][k]
                    else:rule=self.rules[kind].get(k)
                    if not rule:return False
                    accepted=rule['input']
                    if 'all' not in accepted:candidates.intersection_update(accepted);constrained=True
                    if k not in self.rules['scopes'] and children(item):return False
            return True
        if 'macro_parts' in node or not gather(children(node)) or not constrained or not candidates:return None
        return '|'.join(sorted(candidates))

    def boolean(self,n):
        if self.scalar(n) not in {'yes','no'}:self.emit('value',n.get('key','value')+' expects yes/no',n)

    def creation_contract(self,k,n,ctx):
        from stellar import children,Context,fields
        contracts={
          'spawn_system':({'initializer'},{'min_jumps','max_jumps','min_distance','max_distance','hyperlane','is_discovered','effect','min_orientation_angle','max_orientation_angle','direction','authorize_spawn_on_galactic_core'}),
          'move_system':({'target'},{'min_jumps','max_jumps','min_distance','max_distance','hyperlane','reset_terra_incognita','direction'}),
          'spawn_megastructure':({'type'},{'name','owner','planet','coords_from','graphical_culture','orbit_distance','orbit_angle','init_effect','random_pos'}),
          'create_starbase':(set(),{'owner','size','design','module','building','effect'}),
        }
        if k not in contracts:return False
        required,optional=contracts[k];f=fields(n)
        for missing in required-f.keys():self.emit('missing-field',k+' requires '+missing,n)
        for child in children(n):
            key=child.get('key','');value=child.get('value')
            if key not in required|optional:self.emit('unknown-field',k+': unknown parameter '+key,child)
            elif key in {'effect','init_effect'}:
                if self.require_block(child):self.script(children(child),'effects',ctx.push({'spawn_system':'galactic_object','spawn_megastructure':'megastructure','create_starbase':'starbase'}[k]))
            elif key in {'min_jumps','max_jumps','min_distance','max_distance','min_orientation_angle','max_orientation_angle','orbit_angle','orbit_distance'}:self.numeric(child,ctx)
            elif key in {'hyperlane','is_discovered','reset_terra_incognita','random_pos','authorize_spawn_on_galactic_core'}:self.boolean(child)
            elif key=='direction':
                if self.scalar(child) not in {'rimwards','left','right','in_system'}:self.emit('value','direction expects rimwards/left/right/in_system',child)
            elif key=='initializer':self.ref('solar_system_initializers',self.scalar(child),child)
            elif key=='owner':self.compatible(self.resolve(self.scalar(child) or '',ctx,child).current,['country'],child,k+' owner')
            elif key in {'target','planet','coords_from'}:
                accepted=['planet','galactic_object','ship'] if k=='move_system' else (['planet'] if key=='planet' else ['planet','colony','galactic_object','ship','fleet','megastructure','starbase'])
                self.compatible(self.resolve(self.scalar(child) or '',ctx,child).current,accepted,child,k+' '+key)
            elif key in {'size','module','building','type'}:
                category={'size':'ship_sizes','module':'starbase_modules','building':'starbase_buildings','type':'megastructures'}[key]
                self.ref(category,self.scalar(child),child)
            else:self.scalar(child)
        self.checked['native_creation_contracts']+=1
        return True

    def initializer(self,n,ctx,planet=False,category='solar_system_initializers'):
        from stellar import children,values
        numeric={'size','orbit_distance','orbit_angle','orbit_distance_from_parent','orbit_angle_offset','orbit_distance_offset','count','max_instances','spawn_chance','scaled_spawn_chance','planet_count','radius','min_orientation_angle','max_orientation_angle','change_orbit'}
        booleans={'home_planet','has_ring','primitive_system','prevent_anomalies','is_starting_location','is_colonizable','mandatory_neighbors','ignore_difficulty','trigger_home_system','starting_planet','has_independent_orbital_line'}
        lists={'flags','planet_flags'}
        scalar={'name','usage','type','initializer','init_effects','ideal_planet_class','graphical_culture','entity','entity_scale','orbit_pattern','neighbor_system','satellite_naming_policy'}
        for child in children(n):
            k=child.get('key','')
            if k.startswith('@'):continue
            if k=='inline_script':self.expand_inline(child,'initializer',ctx,handler=lambda body:self.initializer(body,ctx,planet));continue
            if k in {'planet','moon'}:
                if self.require_block(child):self.initializer(child,ctx.push('planet'),True)
            elif k=='init_effect':
                if self.require_block(child):self.script(children(child),'effects',ctx)
            elif k in numeric:self.numeric_range(child)
            elif k in booleans:self.boolean(child)
            elif k in lists:
                if self.require_block(child):
                    for tag in children(child):
                        if 'key' in tag or 'children' in tag:self.emit('structure',k+' expects a scalar list',tag)
            elif k=='class':
                value=self.scalar(child)
                if planet and value in {'star','none','ideal_planet_class','ideal_design_class','random','random_non_ideal','random_colonizable','random_non_colonizable','random_asteroid'}:continue
                if planet and value and value.startswith('rl_') and value not in self.entities['planet_classes']:
                    self.gap('initializer random class selector is not in exported database: '+value,child);continue
                self.ref('planet_classes' if planet else 'star_classes',value,child)
            elif k=='modifier':self.ref('static_modifiers',self.scalar(child),child)
            elif k=='anomaly':self.ref('anomalies',self.scalar(child),child)
            elif k in {'modifiers','deposit_blockers'}:
                if child.get('value')=='none':continue
                if self.require_block(child):
                    for value in values(child):self.ref('static_modifiers' if k=='modifiers' else 'deposits',value,child)
                    if any('key' in x or 'children' in x for x in children(child)):self.gap('initializer complex '+k,child)
            elif k=='usage':
                if self.scalar(child) not in {'empire_init','fallen_empire_init','misc_system_init','custom_empire','origin','nomad_init'}:self.emit('value','invalid initializer usage',child)
            elif k in {'usage_odds','spawn_weight'}:self.weighted(child,'triggers',ctx)
            elif k=='neighbor_system':
                if self.require_block(child):
                    for sub in children(child):
                        key=sub.get('key','')
                        if key=='initializer':self.ref('solar_system_initializers',self.scalar(sub),sub)
                        elif key in {'distance','hyperlane_distance','hyperlane_jumps','min_orientation_angle','max_orientation_angle','spawn_chance'}:self.numeric_range(sub)
                        elif key=='trigger':self.script(children(sub),'triggers',ctx)
                        elif key in booleans:self.boolean(sub)
                        else:self.gap('initializer neighbor subgrammar: '+key,sub)
            elif k in {'asteroid_belt','orbital_line'}:
                if self.require_block(child):
                    for sub in children(child):
                        if sub.get('key') in {'radius','orbit_distance','orbit_distance_from_parent'}:self.numeric_range(sub)
                        elif sub.get('key')=='type':self.scalar(sub)
                        else:self.emit('unknown-field','unknown '+k+' field '+sub.get('key',''),sub)
            elif k in scalar:self.scalar(child)
            elif not self.observed_field(category,child,ctx):self.emit('unknown-field','initializer: unknown field '+k,child)
        self.checked['initializer_declarations']+=1

    def civic_condition(self,n,ctx):
        from stellar import children
        references={'ethics':'ethics','authority':'governments/authorities','civics':'governments/civics','origin':'governments/civics','species_class':'species_classes','species_archetype':'species_archetypes','traits':'traits','preferred_planet_class':'planet_classes','country_type':'country_types'}
        for child in children(n):
            k=child.get('key','');low=k.lower()
            if low in {'and','or','not','nor','nand'}:
                if self.require_block(child):self.civic_condition(child,ctx)
            elif k in references:
                def walk(node):
                    for item in children(node):
                        if item.get('key','').lower() in {'and','or','not','nor','nand'}:walk(item)
                        elif item.get('key') in {'value','text'}:
                            if item['key']=='value':self.ref(references[k],self.scalar(item),item)
                            else:self.localisation(item)
                        else:self.emit('unknown-field','unknown empire condition field '+item.get('key',''),item)
                if self.require_block(child):walk(child)
            else:self.script([child],'triggers',ctx)

    def index_initializer_roots(self,definitions):
        """Neighbors inherit their empire initializer's ROOT through the tree."""
        from stellar import fields,children
        names={name for name,node in definitions.items() if fields(node).get('usage',{}).get('value') in {'empire_init','fallen_empire_init','custom_empire','origin','nomad_init'} or any(fields(p).get('home_planet',{}).get('value')=='yes' for p in children(node) if p.get('key')=='planet')}
        def neighbors(node):
            for item in children(node):
                if item.get('key')=='neighbor_system':
                    yield from (x.get('value') for x in children(item) if x.get('key')=='initializer')
                yield from neighbors(item)
        while True:
            updated=names|{child for name in names if name in definitions for child in neighbors(definitions[name]) if child in definitions}
            if updated==names:break
            names=updated
        self.empire_initializers=names

    def added_declaration(self,category,n):
        if self.handoff_definition(category,n):return True
        from stellar import children,Context,fields,values
        if category=='solar_system_initializers':
            empire=fields(n).get('usage',{}).get('value') in {'empire_init','fallen_empire_init','custom_empire','origin','nomad_init'}
            empire=empire or n.get('key') in getattr(self,'empire_initializers',set())
            empire=empire or any(fields(p).get('home_planet',{}).get('value')=='yes' for p in children(n) if p.get('key')=='planet')
            # The system-tree example supplies a system root. Empire-specific
            # initialization also has country-root native callers; keep this
            # distinction explicit instead of declaring all roots a country.
            root='country|galactic_object' if empire else 'galactic_object'
            self.initializer(n,Context('galactic_object',root));return True
        if category not in {'bypass','governments/civics'}:return False
        ctx=Context('country','country');known=self.declarations.get(category,{})
        for child in children(n):
            k=child.get('key','')
            if k.startswith('@'):continue
            if k=='inline_script':self.expand_inline(child,'definition',ctx,category);continue
            if category=='bypass' and k=='connection_type':
                if self.scalar(child) not in {'any_other','one_to_one','scripted_connection','hyper_relay','to_any_system'}:self.emit('value','invalid bypass connection_type',child)
            elif k in {'prerequisites','initializers'}:
                if self.require_block(child):
                    for value in values(child):self.ref('technology' if k=='prerequisites' else 'solar_system_initializers',value,child)
                    if any('key' in x for x in children(child)):self.emit('structure',k+' expects a scalar list',child)
            elif k=='starting_colony':self.ref('planet_classes',self.scalar(child),child)
            elif k in {'playable','ai_playable','possible','potential','allow','country_can_use'}:
                if self.require_block(child):self.civic_condition(child,ctx) if category=='governments/civics' else self.script(children(child),'triggers',ctx)
            elif k in {'random_weight','ai_weight'}:self.weighted(child,'triggers',ctx)
            elif k.startswith('on_') and 'children' in child:self.script(children(child),'effects',ctx)
            elif not self.observed_field(category,child,ctx):self.emit('unknown-field',category+': unknown field '+k,child)
        self.checked['typed_declarations']+=1
        return True

    def declaration_subgrammar(self,category,child,ctx):
        if self.ship_field(category,child,ctx):return True
        if self.handoff_field(category,child,ctx):return True
        from stellar import children,fields
        k=child.get('key','')
        if category=='special_projects' and k=='desc':
            # Installed 06_projects_nomads explicitly documents desc THIS as
            # the disabled vessel, matching event_scope. ROOT/FROM unspecified.
            from stellar import Context,UNKNOWN
            self.description(child,Context(ctx.current,UNKNOWN));return True
        if category=='special_projects' and k=='triggered_requirement':
            from stellar import Context,UNKNOWN
            if not self.require_block(child):return True
            for missing in {'text','trigger'}-fields(child).keys():self.emit('missing-field','triggered_requirement requires '+missing,child)
            for item in children(child):
                key=item.get('key')
                if key=='text':self.localisation(item)
                elif key=='count':self.numeric(item,ctx)
                elif key=='trigger':
                    if self.require_block(item):self.script(children(item),'triggers',Context('fleet',UNKNOWN))
                else:self.emit('unknown-field','unknown triggered_requirement field '+str(key),item)
            return True
        if category=='pop_jobs' and k=='swappable_data':
            if not self.require_block(child):return True
            for variant in children(child):
                if variant.get('key')=='trigger':self.script(children(variant),'triggers',ctx);continue
                if not self.require_block(variant):continue
                for item in children(variant):
                    key=item.get('key','')
                    if key in {'trigger','potential'}:
                        from stellar import Context
                        self.script(children(item),'triggers',Context('planet|colony','planet|colony'))
                    elif key=='building_icon':
                        value=self.scalar(item)
                        if value not in self.entities['buildings'] and not self.building_icon_asset(value,item):self.ref('buildings',value,item)
                    elif key in {'icon','name','description','desc','condition_string'}:self.scalar(item)
                    elif key=='weight':self.numeric(item,ctx) if 'value' in item else self.weighted(item,'triggers',ctx)
                    elif key=='category':self.ref('pop_categories',self.scalar(item),item)
                    else:self.emit('unknown-field','unknown job swappable_data field '+key,item)
            return True
        if category=='special_projects' and k=='requirements':
            if not self.require_block(child):return True
            numeric={'skill','carries_colony','shipclass_military','fleet_power','fleet_size','shipclass_constructor','shipclass_science_ship','shipclass_colonizer','shipclass_transport','assault_armies','defense_armies'}
            for item in children(child):
                key=item.get('key','')
                if key in numeric:self.numeric(item,ctx)
                elif key=='leader':self.ref('leader_classes',self.scalar(item),item)
                elif key=='has_trait':self.ref('traits',self.scalar(item),item)
                elif key=='has_technology':self.ref('technology',self.scalar(item),item)
                elif key=='handled_by_event_tooltip':self.localisation(item)
                elif key in {'is_founder_species','research_station','mining_station','observation_station'}:self.boolean(item)
                elif key=='size_requirement':
                    if self.require_block(item):
                        for sub in children(item):
                            if sub.get('key')=='ship_size':self.ref('ship_sizes',self.scalar(sub),sub)
                            elif sub.get('key')=='count':self.numeric(sub,ctx)
                            else:self.emit('unknown-field','unknown size_requirement field '+sub.get('key',''),sub)
                else:self.emit('unknown-field','unknown project requirement '+key,item)
            return True
        return False
