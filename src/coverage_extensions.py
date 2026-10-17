"""Small engine-exported subgrammars; unsupported forms remain explicit gaps."""
class CoverageExtensions:
 def extended_parameter(self,k,n,kind,ctx):
  from stellar import children,fields
  if k=='queue_actions':self.fleet_actions(n,ctx);return True
  if k in {'start_situation','auto_move_to_planet'}:
   required='type' if k=='start_situation' else 'target'
   if required not in fields(n):self.emit('missing-field',k+' requires '+required,n)
   for c in children(n):
    key=c.get('key')
    if self.extended_child(k,c,ctx):continue
    if key=='type' and k=='start_situation':self.ref('situations',self.scalar(c),c)
    elif key=='target':
     value=self.scalar(c)
     if value:
      resolved=self.resolve(value,ctx,c)
      if k=='auto_move_to_planet':self.compatible(resolved.current,['planet'],c,'auto_move_to_planet target')
    elif key=='clear_auto_move_on_arrival' and k=='auto_move_to_planet':self.boolean(c)
    else:self.gap('undocumented parameter: '+k+'/'+str(key),c,'rule-unimplemented')
   return True
  if k=='fire_on_action':
   f=fields(n)
   if 'on_action' not in f:self.emit('missing-field','fire_on_action requires on_action',n)
   for c in children(n):
    if c.get('key')=='on_action':self.scalar(c)
    elif c.get('key')=='scopes':
     if self.require_block(c):
      import re
      for x in children(c):
       if not re.fullmatch('(from)+',x.get('key','').lower()):self.emit('unknown-field','on_action scope override expects from/fromfrom/...',x)
       elif self.scalar(x):self.resolve(x['value'],ctx,x)
    else:self.emit('unknown-field','unknown fire_on_action field '+str(c.get('key')),c)
   return True
  if k=='has_active_event':
   if self.require_block(n):
    for c in children(n):
     if 'key' in c or 'children' in c:self.emit('structure','has_active_event expects event ID list',c)
     elif self.scalar(c) not in self.events:self.emit('missing-event','unknown event '+str(c.get('value')),c)
   return True
  if k=='calc_true_if':
   if self.require_block(n):
    if 'amount' not in fields(n):self.emit('missing-field','calc_true_if requires amount',n)
    for c in children(n):
     if c.get('key')=='amount':self.numeric(c,ctx)
     else:self.script([c],'triggers',ctx)
   return True
  return False

 def extended_child(self,k,c,ctx):
  from stellar import children,fields
  key=c.get('key')
  if k=='set_agreement_terms' and key in self.entities.get('agreement_terms',set()):
   value=self.scalar(c)
   if value:self.ref('agreement_term_values',value,c)
   if value in self.definitions.get('agreement_term_values',{}):
    term=fields(self.definitions['agreement_term_values'][value]).get('term',{}).get('value')
    if term!=key:self.emit('value','agreement term value '+value+' belongs to '+str(term)+', not '+key,c)
   return True
  if k=='create_country' and key=='name' and 'children' in c:
   if 'key' not in fields(c):self.emit('missing-field','country name block requires key',c)
   for x in children(c):
    a=x.get('key')
    if a=='key':self.localisation(x)
    elif a=='variable_string':self.scalar(x) # runtime text substitution
    else:self.gap('country name substitution grammar not implemented: '+str(a),x,'rule-unimplemented')
   return True
  if k=='modify_species' and key in {'add_trait','remove_trait'}:
   self.ref('traits',self.scalar(c),c);return True
  if k=='start_situation' and key=='effect':
   if self.require_block(c):self.script(children(c),'effects',ctx.push('situation'))
   return True
  if k=='auto_move_to_planet' and key=='arrival_effect':
   callback=ctx
   if ctx.current!='fleet':
    reason='scope-type-unknown' if ctx.current=='?' else 'engine-contract-unverified'
    self.gap('arrival_effect callback contract is not verified for '+ctx.current,c,reason,{'input_scope':ctx.current,'verified_callback_input':['fleet']});callback=ctx.push('?')
   if 'children' in c:self.script(children(c),'effects',callback)
   elif self.scalar(c):
    name=c['value']
    if name not in self.scripts['effects']:self.emit('missing-item','unknown arrival scripted effect '+name,c)
    else:self.expand(name,{**c,'key':name,'value':'yes'},'effects',callback)
   return True
  if k=='set_fleet_settings' and key=='garrison':self.boolean(c);return True
  if k=='create_country' and key=='ethos':
   if 'value' in c:
    if self.scalar(c) not in {'random','none'}:self.compatible(self.resolve(c['value'],ctx,c).current,['country'],c,'ethos source')
   elif self.require_block(c):
    for x in children(c):
     if x.get('key')!='ethic':self.emit('unknown-field','ethos expects ethic entries',x)
     elif self.scalar(x)!='random':self.ref('ethics',x.get('value'),x)
   return True
  if k=='create_country' and key=='flag':
   if 'value' in c:
    value=self.scalar(c)
    if value not in {'random','none'}:self.compatible(self.resolve(value,ctx,c).current,['country'],c,'flag source')
    return True
   if not self.require_block(c):return True
   for x in children(c):
    a=x.get('key')
    if a in {'icon','background'}:
     if self.require_block(x):
      f=fields(x)
      for missing in {'category','file'}-f.keys():self.emit('missing-field','flag image requires '+missing,x)
      for p in children(x):
       if p.get('key') not in {'category','file'}:self.emit('unknown-field','unknown flag image field '+str(p.get('key')),p)
       else:self.scalar(p)
      if all(f.get(t,{}).get('value') for t in ['category','file']):self.asset_reference('flags/'+f['category']['value']+'/'+f['file']['value'],x)
    elif a=='colors':
     if self.require_block(x):
      if len(children(x))!=4:self.emit('structure','flag colors requires four entries',x)
      for p in children(x):
       if 'key' in p or 'children' in p:self.emit('structure','flag colors expects scalar list',p)
       elif self.scalar(p)!='null':self.ref('flag_colors',p.get('value'),p)
    elif a=='use_ship_color':self.boolean(x)
    else:self.emit('unknown-field','unknown flag field '+str(a),x)
   return True
  if k=='create_species' and key=='traits' and 'children' in c:
   for x in children(c):
    a=x.get('key');v=self.scalar(x)
    if a=='ideal_planet_class':
     if v and v.startswith('pc_'):self.ref('planet_classes',v,x)
     elif v:self.compatible(self.resolve(v,ctx,x).current,['planet','species'],x,'ideal_planet_class source')
    elif a in {None,'trait'}:
     if v not in {'random_traits','random'}:self.ref('traits',v,x)
    else:self.emit('unknown-field','unknown species traits field '+str(a),x)
   return True
  if k=='create_leader' and key=='traits':
   if self.require_block(c):
    for x in children(c):
     if 'key' not in x:self.emit('structure','leader traits requires level = trait',x)
     else:
      value=self.scalar(x)
      if value and value not in {'random','random_traits'}:
       # Special add_trait tokens have a separate engine grammar.
       if value in self.entities['traits']:self.ref('traits',value,x)
       elif value.startswith('leader_trait_'):self.ref('traits',value,x)
       else:self.gap('leader trait special token contract: '+value,x)
   return True
  if k=='give_specimen' and key=='targets':
   if self.require_block(c):
    for x in children(c):
     if 'key' in x or 'children' in x:self.emit('structure','specimen targets expects scope list',x)
     elif self.scalar(x):self.resolve(x['value'],ctx,x)
   return True
  if k=='clone_leader' and key=='skip_background_generation':self.boolean(c);return True
  if k=='create_leader' and key=='tier':self.ref('leader_tiers',self.scalar(c),c);return True
  if k=='add_random_research_option' and key=='fail_effects':
   if self.require_block(c):self.script(children(c),'effects',ctx)
   return True
  if k=='create_message' and key=='variable':
   if self.require_block(c):
    f=fields(c)
    if 'key' in f or 'value' in f:
     for missing in {'key','value'}-f.keys():self.emit('missing-field','message literal variable requires '+missing,c)
     for x in children(c):
      if x.get('key') not in {'key','value'}:self.emit('unknown-field','unknown message literal variable field '+str(x.get('key')),x)
      else:self.scalar(x)
     return True
    for missing in {'type','localization','scope'}-f.keys():self.emit('missing-field','message variable requires '+missing,c)
    for x in children(c):
     a=x.get('key');v=self.scalar(x)
     if a not in {'type','localization','scope'}:self.emit('unknown-field','unknown message variable field '+str(a),x)
     elif a=='scope' and v:self.resolve(v,ctx,x)
     elif a=='type' and v!='name':self.gap('message variable type contract: '+str(v),x)
     # localization is a substitution placeholder, not a localisation key.
   return True
  return False

 def picture_event_data(self,n,ctx):
  from stellar import children
  if not self.require_block(n):return
  for c in children(n):
   k=c.get('key');v=self.scalar(c)
   if k not in {'portrait','planet_background','graphical_culture','city_level','room'}:
    self.gap('picture_event_data unmodeled selector: '+str(k),c);continue
   if not v:continue
   # Installed event callers use countries/leaders as portraits and
   # countries/planets as backgrounds; literal room path is documented in
   # asset_selectors/room_textures.txt. No claim of rendering validation.
   if self.is_scope(v):
    typ=self.resolve(v,ctx,c).current
    accepted={'portrait':{'country','leader'},'planet_background':{'country','planet'},'graphical_culture':{'country'},'city_level':{'country'},'room':{'country'}}[k]
    if set(typ.split('|'))<=accepted:self.checked['picture_country_selectors']+=1
    else:self.gap('picture_event_data selector contract: '+str(k)+'/'+typ,c,'scope-type-unknown' if typ=='?' else 'engine-contract-unverified',{'selector':v,'selector_type':typ,'verified_selector_types':sorted(accepted),'other_forms_verified':False})
   elif k=='portrait':self.ref('gfx_portrait',v,c)
   elif k=='room':self.asset_reference('gfx/portraits/city_sets/'+v+'.dds',c)
   else:self.gap('picture_event_data literal selector: '+str(k),c)
