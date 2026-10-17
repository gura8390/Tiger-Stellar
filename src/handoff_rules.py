"""Typed contracts for the frozen Solar winds 0.3.1 handoff."""
from dataclasses import replace

class HandoffRules:
    def narrow(self,body,ctx,positive=True):
        from stellar import children,UNKNOWN
        from stellar_scopes import ENGINE_SCOPE_TYPES
        possible=(set(ENGINE_SCOPE_TYPES)-{'all','various','no_scope'}) if ctx.current==UNKNOWN else set(ctx.current.split('|'))
        def predicate(item,domain,active=()):
            k=item.get('key','').lower();v=item.get('value')
            if k=='is_scope_type' and v in ENGINE_SCOPE_TYPES:return domain & {v},True
            name=item.get('key','')
            if name in self.scripts['triggers'] and v in {'yes','no'} and name not in active:
                definition,_=self.scripts['triggers'][name]
                if 'macro_parts' not in definition and len(active)<20:
                    parts=[predicate(x,domain,active+(name,)) for x in children(definition)]
                    selected=set.intersection(*(part[0] for part in parts)) if parts else set(domain)
                    exact=all(part[1] for part in parts)
                    if v=='no':return (domain-selected,True) if exact else (set(domain),False)
                    return selected,exact
            if k in {'and','limit','hidden_trigger','custom_tooltip','custom_tooltip_with_modifiers','or','not','nor'}:
                parts=[predicate(x,domain,active) for x in children(item) if x.get('key') not in {'text','fail_text','success_text'}]
                if not parts:return set(domain),False
                result=set.union(*(part[0] for part in parts)) if k in {'or','nor'} else set.intersection(*(part[0] for part in parts))
                exact=all(part[1] for part in parts)
                if k in {'not','nor'}:return (domain-result,True) if exact else (set(domain),False)
                return result,exact
            return set(domain),False
        parts=[predicate(item,possible) for item in body]
        selected=set.intersection(*(part[0] for part in parts)) if parts else set(possible)
        if not positive:selected=possible-selected if parts and all(part[1] for part in parts) else set(possible)
        if ctx.current==UNKNOWN and selected==possible:return ctx
        return replace(ctx,current='|'.join(sorted(selected))) if selected else None

    def handoff_parameter(self,k,n,kind,ctx):
        from stellar import children,fields,values
        if k=='relative_power':
            allowed={'who','value','category'}
            for c in children(n):
                key=c.get('key')
                if key not in allowed:self.emit('unknown-field','relative_power: unknown field '+str(key),c)
                elif key=='who':self.compatible(self.resolve(self.scalar(c) or '',ctx,c).current,['country','federation'],c,'relative_power who')
                elif key=='category':
                    if self.scalar(c) not in {'fleet','economy','technology','all'}:self.emit('value','invalid relative_power category',c)
                elif self.scalar(c) not in {'pathetic','inferior','equivalent','superior','overwhelming'}:self.emit('value','invalid relative_power threshold',c)
            for missing in {'who','value'}-fields(n).keys():self.emit('missing-field','relative_power requires '+missing,n)
            return True
        if k=='join_alliance':
            for c in children(n):
                key=c.get('key')
                if key=='who':self.compatible(self.resolve(self.scalar(c) or '',ctx,c).current,['country','federation'],c,'join_alliance who')
                elif key=='name':self.localisation(c)
                elif key=='override_requirements':self.boolean(c)
                else:self.emit('unknown-field','join_alliance: unknown field '+str(key),c)
            if 'who' not in fields(n):self.emit('missing-field','join_alliance requires who',n)
            return True
        if k=='graphical_culture' and kind=='triggers':
            # This is an enum-valued trigger; boolean combinators apply to its values.
            def walk(node):
                for c in children(node):
                    if c.get('key','').lower() in {'or','nor','and','not'}:walk(c)
                    elif c.get('key')=='text':self.localisation(c)
                    elif c.get('key')=='value':self.ref('graphical_culture',self.scalar(c),c)
                    elif 'key' in c or 'children' in c:self.emit('structure','graphical_culture expects enum values',c)
                    else:self.ref('graphical_culture',c.get('value'),c)
            walk(n);return True
        if k=='change_government':
            for c in children(n):
                key=c.get('key')
                if key=='civics':
                    if c.get('value')=='random':continue
                    if self.require_block(c):
                        for x in children(c):
                            if x.get('key')!='civic':self.emit('unknown-field','civics expects civic entries',x)
                            elif self.scalar(x)!='random':self.ref('governments/civics',x.get('value'),x)
                elif key=='authority':
                    if self.scalar(c)!='random':self.ref('governments/authorities',c.get('value'),c)
                elif key in {'cooldown','remove_invalid_civics'}:self.boolean(c)
                else:self.emit('unknown-field','change_government: unknown field '+str(key),c)
            return True
        if k=='change_country_flag':
            for c in children(n):
                key=c.get('key')
                if key in {'icon','background'}:
                    if self.require_block(c):
                        for x in children(c):
                            if x.get('key') not in {'category','file'}:self.emit('unknown-field','unknown flag image field',x)
                            else:self.scalar(x)
                        for missing in {'category','file'}-fields(c).keys():self.emit('missing-field',key+' requires '+missing,c)
                elif key=='colors':
                    if self.require_block(c):
                        for x in children(c):
                            if 'key' in x or 'children' in x:self.emit('structure','flag colors expects a list',x)
                            elif x.get('value')!='null':self.ref('flag_colors',x.get('value'),x)
                else:self.emit('unknown-field','unknown flag field '+str(key),c)
            return True
        return False

    def handoff_field(self,category,n,ctx):
        from stellar import children,fields,Context,values
        k=n.get('key')
        country=Context('country','country')
        if category=='governments/civics':
            if k=='modifier':self.modifier(n,ctx);return True
            if k=='modification':
                if 'value' in n:self.boolean(n);return True
                if self.require_block(n):
                    for c in children(n):
                        if c.get('key') in {'add','remove'} and self.require_block(c):self.script(children(c),'triggers',country)
                        elif c.get('key')=='moddable_conditions_custom_tooltip':self.localisation(c)
                        else:self.emit('unknown-field','modification expects add/remove conditions',c)
                return True
            if k=='swap_type':
                if self.require_block(n):
                    for c in children(n):
                        key=c.get('key')
                        if key=='trigger':self.script(children(c),'triggers',country)
                        elif key=='modifier':self.modifier(c,country)
                        elif key in {'name','description','negative_description','icon','picture','custom_tooltip','custom_tooltip_with_modifiers'}:self.scalar(c)
                        else:self.gap('civic swap_type field: '+str(key),c)
                return True
            if k=='traits':
                if self.require_block(n):
                    for c in children(n):
                        if c.get('key')=='trait':self.ref('traits',self.scalar(c),c)
                        else:self.emit('unknown-field','traits expects trait entries',c)
                return True
            if k=='has_secondary_species':
                for c in children(n):
                    if c.get('key')=='traits':self.handoff_field(category,c,ctx)
                    elif c.get('key')=='title':self.localisation(c)
                    else:self.gap('secondary species field: '+str(c.get('key')),c)
                return True
            if k=='leader_background_job_weight':
                for c in children(n):self.ref('pop_jobs',c.get('key'),c);self.numeric(c,country)
                return True
        if category=='ship_sizes':
            if k in {'selectable','possible_country'}:
                if self.require_block(n):self.script(children(n),'triggers',country)
                return True
            if k=='min_upgrade_cost':
                for c in children(n):self.ref('strategic_resources',c.get('key'),c);self.numeric(c,country)
                return True
            if k=='section_slots':
                if self.require_block(n):
                    for c in children(n):
                        if self.require_block(c):
                            for x in children(c):
                                if x.get('key') in {'locator','template'}:
                                    if x['key']=='template':self.ref('section_templates',self.scalar(x),x)
                                    else:self.scalar(x)
                                else:self.emit('unknown-field','unknown section slot field '+str(x.get('key')),x)
                return True
            if k=='ai_ship_data':
                for c in children(n):
                    if c.get('key') in {'fraction','weight','weight_modifier'}:self.weighted(c,'triggers',country)
                    elif c.get('key') in {'min','max','min_count','max_count','planet_mult','system_mult'}:self.numeric(c,country)
                    else:self.gap('ai_ship_data field: '+str(c.get('key')),c)
                return True
            if k=='triggered_ship_roles':
                if self.require_block(n):
                    for c in children(n):
                        for x in children(c):
                            if x.get('key')=='trigger':self.script(children(x),'triggers',country)
                            elif x.get('key')=='name':self.scalar(x)
                            else:self.emit('unknown-field','unknown triggered ship role field',x)
                return True
            if k=='space_fauna_values':
                for c in children(n):
                    if c.get('key')=='age_threshold':self.numeric_range(c)
                    elif c.get('key')=='culling_value':self.resources(c,Context('ship_growth_stage','ship_growth_stage'))
                    elif c.get('key') in {'icon_frame','monthly_initial_breeding_chance','monthly_increased_breeding_chance','breeding_cooldown_months','merge_with_individuals_amount_to_upgrade'}:self.numeric(c,country)
                    elif c.get('key')=='can_breed_alone':self.boolean(c)
                    elif c.get('key')=='reanimated_ship_design':self.ref('global_ship_designs',self.scalar(c),c)
                    else:self.gap('space_fauna_values field: '+str(c.get('key')),c)
                return True
            contracts={'map_icon_override':{'normal','mixed','cloaked','encamped'},'hero_ship':{'custom_state_loc_prefix','hero_ship_fleet_background','custom_class_text_color','capabilities'}}
            if k in contracts:
                for c in children(n):
                    if c.get('key') not in contracts[k]:self.emit('unknown-field','unknown '+k+' field',c)
                    elif c.get('key')=='capabilities':
                        if self.require_block(c):
                            for cap in values(c):
                                if cap not in {'can_repair','can_upgrade'}:self.emit('value','unknown hero_ship capability '+cap,c)
                    else:self.scalar(c)
                return True
        return False

    def handoff_definition(self,category,n):
        from stellar import children,Context,fields
        if category not in {'diplomatic_actions','country_types','personalities'}:return False
        ctx=Context('country','country',('country',),('country',))
        schema=self.declarations.get(category,{})
        for c in children(n):
            k=c.get('key','');shape=schema.get(k)
            if k.startswith('@'):continue
            if k=='inline_script':self.expand_inline(c,'definition',ctx,category);continue
            if k in {'potential','possible','proposable','allow','trade_routes_available'}:self.script(children(c),'triggers',ctx)
            elif k in {'on_accept','on_decline','on_send'}:self.script(children(c),'effects',ctx)
            elif k in {'ai_acceptance','weight_modifier','ai_weight','should_ai_propose'}:self.weighted(c,'triggers',ctx)
            elif k in {'resources','branch_office_resources'}:self.resources(c,ctx)
            elif k=='modules':
                for module in children(c):
                    for x in children(module):
                        if x.get('key') in {'can_send','can_receive'}:
                            for a in x.get('children',[]):self.ref('diplomatic_actions',a.get('value'),a)
                        elif x.get('key')=='contact_rule':self.scalar(x)
                        else:self.gap('country module parameter: '+str(x.get('key')),x)
            elif k=='ai':
                for x in children(c):
                    if x.get('key')=='modules':
                        for item in children(x):self.scalar(item)
                    elif x.get('value') in {'yes','no'}:self.boolean(x)
                    elif x.get('key')=='min_assault_armies_for_wars':self.numeric(x,ctx)
                    else:self.gap('country ai field: '+str(x.get('key')),x)
            elif shape and 'value' in c:
                if shape['scalar_types']==['boolean']:self.boolean(c)
                elif shape['scalar_types']==['number']:self.numeric(c,ctx)
                else:self.scalar(c)
            elif not shape:self.emit('unknown-field',category+': unknown field '+k,c)
            else:self.gap('definition subgrammar: '+category+'/'+k,c)
        return True
