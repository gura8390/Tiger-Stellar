"""Typed subset of fleet-action DSL, grounded in installed examples/guide."""
class FleetActions:
 def fleet_actions(self,n,ctx,ids=None):
  from stellar import children,fields,UNKNOWN
  if not self.require_block(n):return
  if ids is None:ids=set()
  def scripted(item,kind,scope):
   if not self.require_block(item):return
   f=fields(item);identity=f.get('id',{}).get('value')
   if not identity:self.emit('missing-field','fleet action '+str(item.get('key'))+' requires save/load id',item)
   elif identity in ids:self.emit('duplicate-field','duplicate fleet action id '+identity,item)
   else:ids.add(identity)
   self.script([c for c in children(item) if c.get('key')!='id'],kind,scope)
  for c in children(n):
   k=c.get('key');v=c.get('value')
   if k in {'move_to','orbit_planet'}:
    value=self.scalar(c)
    if value:self.compatible(self.resolve(value,ctx,c).current,['planet'] if k=='orbit_planet' else ['planet','galactic_object','ship','fleet','starbase','megastructure'],c,'fleet action '+k)
   elif k=='wait':
    if 'value' in c:self.numeric(c,ctx)
    elif self.require_block(c):
     if 'duration' not in fields(c):self.emit('missing-field','wait requires duration',c)
     for part in children(c):
      if part.get('key') in {'duration','random'}:self.numeric(part,ctx)
      else:self.emit('unknown-field','unknown fleet wait field '+str(part.get('key')),part)
   elif k=='effect':
    callback=ctx
    if ctx.current not in {'fleet','?'}:
     self.gap('fleet action effect callback scope is not verified for selected object',c,'engine-contract-unverified',{'selection_scope':ctx.current});callback=ctx.push(UNKNOWN)
    scripted(c,'effects',callback)
   elif k=='repeat':
    if not self.require_block(c):continue
    body=[]
    for part in children(c):
     if part.get('key')=='max_iterations':self.numeric(part,ctx)
     elif part.get('key')=='while':scripted(part,'triggers',ctx)
     else:body.append(part)
    self.fleet_actions({'key':'repeat','children':body},ctx,ids)
   elif k in {'find_closest_planet','find_random_planet','find_random_system','find_closest_system'}:
    if not self.require_block(c):continue
    planet=k.endswith('planet');output='planet' if planet else 'galactic_object';found='found_planet' if planet else 'found_system';selected=ctx.push(output)
    for part in children(c):
     if part.get('key')=='trigger':scripted(part,'triggers',selected)
     elif part.get('key')==found:self.fleet_actions(part,selected,ids)
     elif part.get('key')=='failed':self.fleet_actions(part,ctx,ids)
     else:self.emit('unknown-field','unknown '+k+' action field '+str(part.get('key')),part)
   elif k=='inline_script':self.expand_inline(c,'fleet-actions',ctx,handler=lambda body:self.fleet_actions(body,ctx,ids))
   elif k in self.rules['effects']:self.emit('structure','fleet actions require effect={id=... <effects>}, not a bare '+k,c)
   else:self.gap('fleet action grammar not implemented: '+str(k),c,'rule-unimplemented',{'dsl':'fleet-actions','action':k})
  self.checked['fleet_action_blocks']+=1
