"""On-action event-list structure and documented preset scope propagation."""
class EntryLists:
 def on_action_events(self,n,ctx):
  import math
  from stellar import children,number,Context,UNKNOWN
  if not self.require_block(n):return
  weighted=n.get('key')=='random_events'
  for c in children(n):
   key=c.get('key');value=c.get('value')
   if key=='inline_script':
    self.expand_inline(c,'on-action-events',ctx,handler=lambda body:self.on_action_events({'key':n['key'],'children':children(body)},ctx));continue
   if 'children' in c or value is None:self.emit('structure','on_action event list expects event IDs or weight = event ID',c);continue
   if weighted:
    weight=number(key)
    if weight is None or not math.isfinite(weight) or weight<0:self.emit('value','random_events expects finite non-negative numeric weights',c);continue
    if value=='0':continue  # documented native no-event entry
   elif key is not None:self.emit('structure','events expects an unkeyed event ID list',c);continue
   if value not in self.events:self.emit('missing-event','unknown event '+value,c);continue
   if weighted and weight==0:continue
   scope=self.events[value];froms=ctx.froms
   if ctx.current!=UNKNOWN and not set(ctx.current.split('|'))&set(scope.split('|')):
    # No verified conversion contract; do not transport preset FROM facts
    # through a different event-root kind.
    froms=()
    self.gap('on_action event root adaptation contract is not verified',c,'engine-contract-unverified',{'on_action_this':ctx.current,'event_root_type':scope,'event':value})
   if not self.inference_mode:continue
   entry=Context(scope,UNKNOWN if scope=='none' else scope,froms)
   record={'context':entry,'targets':{},'caller':self.file,'line':c.get('line',0),'source_kind':'on_action preset (no scripted-event FROM shift)','engine_input_context':{'this':ctx.current,'root':ctx.root,'froms':list(ctx.froms)}}
   identity=('on-action',self.file,c.get('line',0),value,entry)
   self.next_event_contexts.setdefault(value,{})[identity]=record
  self.checked['on_action_event_lists']+=1
