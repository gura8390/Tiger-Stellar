"""Demand-driven caller/type flow. No runtime existence or invocation proof."""
import collections,re

class ContextFlow:
 def discovery_roots(self,target):
  from stellar import children
  scripts=set().union(*(set(group) for group in self.scripts.values()))
  def tokens(node):
   result={node.get('key','')}
   if node.get('key')=='arrival_effect' and node.get('value') in scripts:result.add(node['value'])
   if 'macro_parts' in node:
    text=''.join(part['text'] for part in node['macro_parts'])
    result.update(re.findall(r'(?m)^\s*([\w.]+)\s*=',text))
   for c in children(node):result.update(tokens(c))
   return result
  graph={name:tokens(n)&scripts for group in self.scripts.values() for name,(n,_) in group.items()}
  reached=set()
  for _,cat,ast in target:
   if cat not in {'scripted_effects','scripted_triggers'}:
    for n in ast:reached.update(tokens(n)&scripts)
  while True:
   added=set().union(*(graph.get(k,set()) for k in reached)) if reached else set()
   if added<=reached:break
   reached.update(added)
  needed={name for group in self.scripts.values() for name,(_,p) in group.items() if p in self.mod_files}-reached
  while True:
   added={k for k,callees in graph.items() if callees&needed}
   if added<=needed:break
   needed.update(added)
  wanted_events={name for name,(_,p) in self.event_defs.items() if p in self.mod_files}
  referenced=set()
  for path,cat,ast in target:
   if cat in {'gfx','interface'}:continue
   def refs(n):
    for value in [n.get('key',''),n.get('value','')]:referenced.update(re.findall(r'(?:global_event_target|event_target):([\w]+)',value))
    if 'macro_parts' in n:referenced.update(re.findall(r'(?:global_event_target|event_target):([\w]+)',''.join(p['text'] for p in n['macro_parts'])))
    for c in children(n):refs(c)
   for n in ast:refs(n)
  def supplies(n):
   k=n.get('key','');v=n.get('value','')
   if k=='save_global_event_target_as' and v.split('@')[0] in referenced:return True
   if k.endswith('_event') and any(c.get('key')=='id' and c.get('value') in wanted_events for c in children(n)):return True
   if k in {'events','random_events'} and any(c.get('value') in wanted_events for c in children(n)):return True
   return any(supplies(c) for c in children(n))
  # Writer scripts need their callers, not an invented standalone context.
  writer_scripts={name for group in self.scripts.values() for name,(n,_) in group.items() if supplies(n)}
  needed.update(writer_scripts)
  while True:
   added={k for k,callees in graph.items() if callees&needed}
   if added<=needed:break
   needed.update(added)
  roots=[];target_ids={(p,cat,n.get('line',0),n.get('key')) for p,cat,ast in target for n in ast}
  for path,cat,ast in getattr(self,'file_records',target):
   if cat in {'scripted_effects','scripted_triggers','gfx','interface'}:continue
   selected=[n for n in ast if (tokens(n)&needed or supplies(n)) and (path,cat,n.get('line',0),n.get('key')) not in target_ids]
   if selected:roots.append((path,cat,selected))
  self.discovery_summary={'root_declarations':sum(len(ast) for _,_,ast in roots),'root_files':len(roots),'purpose':'infer caller/target types only; upstream bodies are not extra validation targets','runtime_availability_proven':False}
  self.discovery_needed=needed
  return roots

 def remember_script_context(self,kind,name,node,ctx):
  if not self.inference_mode or name in self.active:return
  key=(kind,name);records=self.script_contexts.setdefault(key,{})
  target_key=tuple(sorted(self.targets.items()))
  records[(ctx,self.file,node.get('line',0),target_key)]={'context':ctx,'caller':self.file,'line':node.get('line',0),'stack':list(self.active),'targets':dict(self.targets),'call':node}

 def remember_event_call(self,name,node,ctx):
  from stellar import Context,UNKNOWN,fields,children
  if not self.inference_mode or name not in self.events:return
  scope=self.events[name]
  # Script-fired events move caller ROOT into FROM and shift the FROM stack.
  # Explicit scopes overrides are evaluated in the caller's frame.
  froms=[ctx.root,*ctx.froms][:6];root=UNKNOWN if scope=='none' else scope
  for override in children(fields(node).get('scopes',{})):
   key=override.get('key','').lower();value=override.get('value')
   if not value:continue
   typ=self.resolve(value,ctx,override).current
   if re.fullmatch(r'(from)+',key):
    index=key.count('from')-1
    if index>=6:continue
    froms.extend([UNKNOWN]*max(0,index+1-len(froms)));froms[index]=typ
   elif key=='root':root=typ
  entry=Context(scope,root,tuple(froms));key=(self.file,node.get('line',0),tuple(self.active),entry,tuple(sorted(self.targets.items())))
  self.next_event_contexts.setdefault(name,{})[key]={'context':entry,'targets':dict(self.targets),'caller':self.file,'line':node.get('line',0)}

 def joined_event_contexts(self):
  from stellar import Context,UNKNOWN
  def union(types):
   types=set(types)
   return UNKNOWN if UNKNOWN in types else '|'.join(sorted(set().union(*(set(t.split('|')) for t in types))))
  joined={}
  for name,records in self.next_event_contexts.items():
   rows=list(records.values());depth=max(len(r['context'].froms) for r in rows)
   ctx=Context(union(r['context'].current for r in rows),union(r['context'].root for r in rows),tuple(union(r['context'].froms[i] if len(r['context'].froms)>i else UNKNOWN for r in rows) for i in range(depth)))
   names=set().union(*(set(r['targets']) for r in rows));targets={k:union(r['targets'][k] for r in rows if k in r['targets']) for k in sorted(names)}
   joined[name]={'merged':{'context':ctx,'targets':targets,'caller':'<joined scripted callers>','line':0}}
  self.event_call_evidence=self.next_event_contexts
  return joined

 def event_variants(self,name,scope,triggered):
  from stellar import Context,UNKNOWN
  variants=list((self.event_contexts if self.inference_mode else getattr(self,'event_call_evidence',self.event_contexts)).get(name,{}).values())
  unique={}
  for r in variants:unique.setdefault((r['context'],tuple(sorted(r['targets'].items()))),r)
  variants=list(unique.values())
  if not variants or not triggered:variants.append({'context':Context(scope,UNKNOWN if scope=='none' else scope),'targets':{},'caller':None,'line':0})
  return sorted(variants,key=lambda x:(repr(x['context']),x['caller'] or '',x['line'],repr(sorted(x['targets'].items()))))

 def check_script_contexts(self,kind,name,node):
  records=self.script_contexts.get((kind,name),{})
  if not records:return False
  if any('call' not in r for r in records.values()):return False
  for record in sorted(records.values(),key=lambda r:(repr(r['context']),r['caller'],r['line'])):
   ctx=record['context'];self.targets=dict(record['targets']);saved=self.file;self.file=record['caller']
   try:self.expand(name,record['call'],kind,ctx)
   finally:self.file=saved
  return True

 def flow_report(self):
  def context(ctx):return {'this':ctx.current,'root':ctx.root,'froms':list(ctx.froms),'prevs':list(ctx.prevs)}
  return {'discovery':getattr(self,'discovery_summary',{}),'event_inputs':{k:[{'context':context(r['context']),'caller':r['caller'],'line':r['line'],'target_types':r['targets'],'runtime_availability_proven':False,'source_kind':r.get('source_kind','scripted event call'),'engine_input_context':r.get('engine_input_context')} for r in sorted(v.values(),key=lambda r:(repr(r['context']),r['caller'],r['line']))] for k,v in sorted(getattr(self,'event_call_evidence',self.event_contexts).items())},'script_inputs':{kind+':'+name:[{'context':context(r['context']),'caller':r['caller'],'line':r['line']} for r in sorted(records.values(),key=lambda r:(repr(r['context']),r['caller'],r['line']))] for (kind,name),records in sorted(self.script_contexts.items()) if self.scripts[kind][name][1] in self.mod_files},'from_depth_bound':6,'runtime_availability_proven':False}
