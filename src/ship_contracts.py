"""Bounded ship contracts. No balance limits and no automatic-design promise.

Sources: installed component_slots, section/ship definitions, weapon CSV and
common/global_ship_designs/000_documentation.txt. Conditional eligibility is
three-valued; unknown conditions never become an eligibility proof.
"""
import math

def val(n,k,default=None):
    from stellar import fields
    return fields(n).get(k,{}).get('value',default)

def hull_condition(nodes,hull):
    """Only a hull test and boolean connectives have a known truth value."""
    from stellar import children
    result=[]
    for n in nodes:
        k=n.get('key','').lower();v=n.get('value')
        if k=='is_ship_size':r=(v==hull)
        elif k=='always':r=True if v=='yes' else False if v=='no' else None
        elif k in {'and','or','not','nor','nand'}:
            parts=[hull_condition([x],hull) for x in children(n)]
            if k in {'or','nor'}:r=True if True in parts else None if None in parts else False
            else:r=False if False in parts else None if None in parts else True
            if k in {'not','nor','nand'}:r=None if r is None else not r
        else:r=None
        result.append(r)
    return False if False in result else None if None in result else True

class ShipContracts:
    def slot_definition(self,n):
        from stellar import children,fields
        for missing in {'size','component'}-fields(n).keys():self.emit('missing-field','component slot requires '+missing,n)
        for c in children(n):
            k=c.get('key');v=c.get('value')
            if k=='size':
                if v not in {'small','medium','large','extra_large','titanic','planet_killer','point_defence','torpedo','engulf','any','aux'}:self.emit('value','unknown component slot size '+str(v),c)
            elif k=='component':
                if v not in {'weapon','utility','strike_craft','any'}:self.emit('value','component slot expects weapon/utility/strike_craft/any',c)
            elif k=='entities':
                if self.require_block(c):
                    for x in children(c):self.scalar(x)
            else:self.gap('component slot template field: '+str(k),c)
        self.checked['component_slot_definitions']+=1

    def strict_range(self,n,ctx):
        from stellar import children,fields,number
        if not self.require_block(n):return
        f=fields(n)
        for missing in {'min','max'}-f.keys():self.emit('missing-field',n['key']+' requires '+missing,n)
        seen=set()
        for c in children(n):
            k=c.get('key')
            if k not in {'min','max'}:self.emit('unknown-field',n['key']+': unknown range field '+str(k),c)
            else:
                if k in seen:self.emit('duplicate-field','duplicate range endpoint '+k,c)
                seen.add(k);self.numeric(c,ctx)
                value=number(c.get('value'))
                if value is not None and (not math.isfinite(value) or value<0):self.emit('value',n['key']+' endpoints must be finite and nonnegative',c)
        lo=number(f.get('min',{}).get('value'));hi=number(f.get('max',{}).get('value'))
        # Native planet-killer windup uses min=90/max=0; do not impose damage
        # ordering on this separate timing structure without an engine contract.
        if n.get('key')=='damage' and lo is not None and hi is not None and lo>hi:self.emit('range-order',n['key']+' min exceeds max',n)
        if lo is None or hi is None:self.gap('dynamic '+n['key']+' bounds cannot be ordered statically',n)
        self.checked['weapon_ranges']+=1

    def ship_field(self,category,n,ctx):
        from stellar import children,fields,Context
        if category!='component_templates':return False
        k=n.get('key')
        if k in {'damage','windup'}:self.strict_range(n,ctx);return True
        if k in {'friendly_aura','hostile_aura'}:
            if not self.require_block(n):return True
            f=fields(n);target='ship' if val(n,'apply_on','ships')=='ships' else 'fleet'
            aura_ctx=Context(target,ctx.root,ctx.froms,ctx.prevs)
            for c in children(n):
                key=c.get('key')
                if key=='name':self.scalar(c)
                elif key=='radius':self.numeric(c,ctx)
                elif key=='system_wide':self.boolean(c)
                elif key=='apply_on':
                    if self.scalar(c) not in {'ships','fleets'}:self.emit('value','aura apply_on expects ships/fleets',c)
                elif key=='stack_info':
                    if self.require_block(c):
                        for missing in {'id','priority'}-fields(c).keys():self.emit('missing-field','stack_info requires '+missing,c)
                        for x in children(c):
                            if x.get('key')=='id':self.scalar(x)
                            elif x.get('key')=='priority':self.numeric(x,ctx)
                            else:self.emit('unknown-field','unknown stack_info field '+str(x.get('key')),x)
                elif key=='modifier':
                    if self.require_block(c):self.modifier(c,aura_ctx)
                elif key=='limit':
                    if self.require_block(c):self.script(children(c),'triggers',aura_ctx)
                else:self.emit('unknown-field','unknown aura field '+str(key),c)
            if 'name' not in f:self.emit('missing-field','aura requires name',n)
            self.checked['component_auras']+=1;return True
        if k=='target_weights':
            if self.require_block(n):
                for c in children(n):self.ref('ship_sizes',c.get('key'),c);self.numeric(c,ctx)
            self.checked['component_target_weights']+=1;return True
        return False

    def weapon_contract(self,n):
        from stellar import fields,number,Context
        f=fields(n);kind=n.get('key');key=val(n,'key')
        if kind=='weapon_component_template':
            if 'cooldown' in f:self.emit('unknown-field','ordinary weapons use total_fire_time, not strike-craft cooldown',f['cooldown'])
            # Vanilla weapons may obtain statistics from the final weapon CSV.
            stats=getattr(self,'weapon_stats',{}).get(key,{})
            for field in ['damage','total_fire_time']:
                if field not in f and not (stats.get('min_damage') and stats.get('max_damage') if field=='damage' else stats.get('cooldown')):
                    self.gap('weapon '+key+' has no inline '+field+' or resolved CSV statistics',n)
            fire=number(val(n,'total_fire_time',stats.get('cooldown')))
            if fire is not None and (not math.isfinite(fire) or fire<=0):self.emit('value','ordinary weapon total_fire_time must be finite and positive',f.get('total_fire_time',n))
        elif kind=='strike_craft_component_template':
            if 'total_fire_time' in f:self.emit('unknown-field','strike craft use cooldown, not ordinary-weapon total_fire_time',f['total_fire_time'])
        self.checked['component_kind_contracts']+=1

    def ship_crosschecks(self,target):
        from reporting import declaration_frame
        from stellar import children,fields,Context
        defs=self.definitions
        sections=defs.get('section_templates',{});hulls=defs.get('ship_sizes',{});comps=defs.get('component_templates',{})
        checked_sections=set()
        for path,category,ast in target:
            self.file=path
            for n in ast:
                with declaration_frame(self,category,n):
                    if category=='section_templates':
                        key=val(n,'key');hull=val(n,'ship_size');h=hulls.get(hull)
                        if h:
                            slot=val(n,'fits_on_slot');slots=fields(fields(h).get('section_slots',{}))
                            if slot and slot not in slots:self.emit('ship-compatibility','section '+str(key)+' does not fit hull section slot '+slot,n)
                        checked_sections.add(key)
                    elif category=='ship_sizes':
                        hull=n.get('key');slots=fields(fields(n).get('section_slots',{}))
                        for slot,spec in slots.items():
                            chosen=val(spec,'template')
                            if chosen in sections:
                                s=sections[chosen]
                                if val(s,'ship_size')!=hull or val(s,'fits_on_slot')!=slot:self.emit('ship-compatibility','section template '+chosen+' does not fit '+hull+'/'+slot,spec)
                        for req in children(n):
                            if req.get('key')!='required_component_set':continue
                            group=req.get('value');candidates=[c for c in comps.values() if val(c,'component_set')==group]
                            possible=[hull_condition(children(fields(c).get('potential',{})),hull) for c in candidates]
                            if not candidates: self.emit('ship-compatibility','required component set '+str(group)+' has no components',req)
                            elif all(x is False for x in possible):self.emit('ship-compatibility','no component in required set '+str(group)+' is applicable to hull '+hull,req)
                            elif True not in possible:self.gap('required set '+str(group)+' for '+hull+' has unresolved component eligibility',req)
        # A cycle cannot have a final upgrade; report only cycles reached from targets.
        for category,idfield in [('ship_sizes',None),('component_templates','key')]:
            for path,cat,ast in target:
                if cat!=category:continue
                self.file=path
                for n in ast:
                    key=val(n,idfield) if idfield else n.get('key');seen=[];cur=key
                    with declaration_frame(self,category,n):
                        while cur in defs.get(category,{}):
                            if cur in seen:self.emit('upgrade-cycle','upgrade cycle: '+' -> '.join(seen+[cur]),n);break
                            seen.append(cur);cur=val(defs[category][cur],'upgrades_to')
        self.ship_feasibility=[]
        for path,category,ast in target:
            if category=='global_ship_designs':
                self.file=path
                for n in ast:
                    with declaration_frame(self,category,n):self.design_contract(n)

    def design_contract(self,n):
        from stellar import children,fields,Context,number
        defs=self.definitions;name=val(n,'name');hulls=defs.get('ship_sizes',{});comps=defs.get('component_templates',{});sections=defs.get('section_templates',{})
        allowed={'name','role','upgrades_to','newborn_ship_design','creature_designer_template','country_type','fleet_specialization_tag','use_design_name','is_event_design','allow_buildable_trigger','is_special_buildable','graphical_culture','growth_stages','ship_size','section','required_component','resource_restriction','custom_military_power','ship_owner_type','custom_design','custom_upgrades_to','custom_newborn_design','auto_gen_design','initial_design'}
        for c in children(n):
            k=c.get('key')
            if k not in allowed:self.gap('global ship design field: '+str(k),c)
            elif k=='allow_buildable_trigger':self.script(children(c),'triggers',Context('country','country'))
            elif k in {'upgrades_to','newborn_ship_design'}:self.ref('global_ship_designs',c.get('value'),c)
        stages=children(fields(n).get('growth_stages',{})) or [n]
        for stage in stages:
            hull=val(stage,'ship_size');self.ref('ship_sizes',hull,stage);h=hulls.get(hull)
            if not h:continue
            selected=[];unknown=[];seen_slots=set()
            for s in children(stage):
                if s.get('key')!='section':continue
                section_key=val(s,'template');self.ref('section_templates',section_key,s);section=sections.get(section_key);slot=val(s,'slot')
                if slot in seen_slots:self.emit('ship-compatibility','duplicate section slot '+str(slot),s)
                seen_slots.add(slot)
                if not section:unknown.append('missing section');continue
                if val(section,'ship_size')!=hull or val(section,'fits_on_slot')!=slot:self.emit('ship-compatibility','section '+str(section_key)+' is incompatible with '+hull+'/'+str(slot),s)
                slots={val(x,'name'):self.definitions.get('component_slot_templates',{}).get(val(x,'template')) for x in children(section) if x.get('key')=='component_slot'}
                for size in ['small','medium','large','aux']:
                    count=number(val(section,size+'_utility_slots','0'))
                    if count is None:unknown.append('dynamic utility slots');continue
                    for i in range(int(count)):slots[size.upper()+'_UTILITY_'+str(i+1)]={'children':[{'key':'size','value':size},{'key':'component','value':'utility'}]}
                occupied=set()
                for c in children(s):
                    if c.get('key')!='component':continue
                    ck=val(c,'template');self.ref('component_templates',ck,c);component=comps.get(ck);cs=val(c,'slot');contract=slots.get(cs)
                    if cs in occupied:self.emit('ship-compatibility','duplicate component slot '+str(cs),c)
                    occupied.add(cs)
                    if cs not in slots:self.emit('ship-compatibility','unknown component slot '+str(cs)+' in section '+str(section_key),c)
                    if component:
                        selected.append(component)
                        if contract:
                            actual={'utility_component_template':'utility','strike_craft_component_template':'strike_craft'}.get(component.get('key'),'weapon')
                            if val(contract,'size') not in {'any',val(component,'size')} or val(contract,'component') not in {'any',actual}:self.emit('ship-compatibility','component '+ck+' has incompatible size/type for slot '+str(cs),c)
                    if contract is None:unknown.append('unresolved slot contract')
                if set(slots)-occupied:unknown.append('unfilled optional component slots')
            required=[x.get('value') for x in children(h) if x.get('key')=='required_component_set'];sets=[]
            for r in children(stage):
                if r.get('key')!='required_component':continue
                key=r.get('value');self.ref('component_templates',key,r);component=comps.get(key)
                if not component:unknown.append('missing core component');continue
                selected.append(component);group=val(component,'component_set');sets.append(group)
                if group not in required:self.emit('ship-compatibility','core component '+key+' is not in a hull required_component_set',r)
            if len(sets)!=len(set(sets)):self.emit('ship-compatibility','duplicate required component set in design',stage)
            if set(required)-set(sets):unknown.append('unfilled required component sets: '+','.join(sorted(set(required)-set(sets))))
            if set(fields(fields(h).get('section_slots',{})))-seen_slots:unknown.append('unfilled hull section slots')
            powers=[]
            for c in selected:
                eligibility=hull_condition(children(fields(c).get('potential',{})),hull)
                if eligibility is False:self.emit('ship-compatibility','component '+val(c,'key')+' is excluded from hull '+hull,stage)
                elif eligibility is None:unknown.append('conditional eligibility: '+val(c,'key'))
                raw=val(c,'power',getattr(self,'weapon_stats',{}).get(val(c,'key'),{}).get('power'))
                power=number(raw)
                if power is None:unknown.append('unknown power: '+val(c,'key'))
                else:powers.append(power)
            net=sum(powers) if len(powers)==len(selected) else None
            power_uncertain=any(x.startswith(('unfilled','unknown power','missing')) for x in unknown)
            # Power modifiers and scripted effects can change the raw component
            # sum. This check does not simulate them or arbitrary auto-fill.
            for c in [h]+selected:
                for block in children(c):
                    if block.get('key','').startswith('triggered_') or any(x.get('key') in {'ship_power_add','ship_power_mult'} for x in children(block)):
                        power_uncertain=True;unknown.append('conditional or modifier-adjusted power')
            if net is not None and net<0 and not power_uncertain:self.emit('ship-power','explicit loadout has negative component power balance: '+str(net),stage)
            item={'design':name,'hull':hull,'net_component_power':net,'status':'unknown' if unknown else ('infeasible' if net is not None and net<0 else 'static_explicit_loadout'),'missing_conditions':sorted(set(unknown)),'automatic_design_validated':False}
            self.ship_feasibility.append(item)
            if unknown:self.gap('explicit ship design feasibility unresolved: '+str(name)+'; '+'; '.join(sorted(set(unknown))),stage)
            self.checked['explicit_ship_designs']+=1
