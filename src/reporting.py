"""Source evidence, targeted native controls and readable coverage summaries."""
import collections,functools,hashlib,json
from contextlib import contextmanager
from pathlib import Path

def canonical(n):
    if isinstance(n,list):return [canonical(x) for x in n]
    if isinstance(n,dict):return {k:canonical(v) for k,v in n.items() if k not in {'line','column','end_line','end_column','offset'} and not k.startswith('_')}
    return n

def digest(n):return hashlib.sha256(json.dumps(canonical(n),ensure_ascii=False,sort_keys=True).encode()).hexdigest()

def identify(n):
    from stellar import fields
    f=fields(n)
    return f.get('id',f.get('key',f.get('name',{}))).get('value',n.get('key',''))

@contextmanager
def declaration_frame(checker,category,node):
    """Cross-file checks retain the same declaration owner as local checks."""
    if checker.inference_mode:
        yield;return
    stack=getattr(checker,'validation_frames',[]);checker.validation_frames=stack
    frame={'kind':'definition','path':node.get('_source_path',checker.file),'line':node.get('line',0),'category':category,'definition':identify(node),'definition_sha256':digest(node)}
    stack.append(frame)
    try:yield
    finally:stack.pop()

def traced(method,kind):
    @functools.wraps(method)
    def invoke(self,*args,**kwargs):
        from stellar import Context
        if self.inference_mode:return method(self,*args,**kwargs)
        node=next((x for x in args if isinstance(x,dict)),{})
        if not node:node=next((x[0] for x in args if isinstance(x,list) and x and isinstance(x[0],dict)),{})
        ctx=next((x for x in args if isinstance(x,Context)),None)
        frame={'kind':kind,'path':node.get('_source_path',self.file),'line':node.get('line',0)}
        if kind=='definition':frame.update(category=args[0],definition=identify(node),definition_sha256=digest(node))
        elif kind=='scripted-call':frame.update(definition=args[0],definition_path=self.scripts[args[2]][args[0]][1],definition_sha256=digest(self.scripts[args[2]][args[0]][0]))
        elif kind=='event-body':frame.update(definition=identify(node),definition_sha256=digest(node))
        if ctx:frame['context']={'this':ctx.current,'root':ctx.root,'froms':list(ctx.froms),'prevs':list(ctx.prevs)}
        stack=getattr(self,'validation_frames',[]);self.validation_frames=stack;stack.append(frame)
        try:return method(self,*args,**kwargs)
        finally:stack.pop()
    return invoke

def annotate(checker,node,record):
    record['statement']={k:node[k] for k in ['key','value','operator'] if k in node}
    record['expression_sha256']=digest(node)
    frames=[dict(x) for x in getattr(checker,'validation_frames',[])]
    record['call_chain']=frames
    record['context']=next((x['context'] for x in reversed(frames) if 'context' in x),None)
    owner=next((x for x in reversed(frames) if 'definition' in x),{})
    record['definition']=owner.get('definition')
    record['definition_sha256']=owner.get('definition_sha256')
    if record['severity'] in {'error','fatal'}:record['analysis']='confirmed_by_implemented_rule'
    elif record['key']=='validation-coverage':record['analysis']='checker_cannot_determine'
    else:record['analysis']='review_required'

def same_contract(a,b):
    # Preserve scope hops and call identities, ignoring only physical locations.
    def contract(path):return json.dumps([(f.get('kind'),f.get('definition'),f.get('context')) for f in path if f.get('context') or f.get('definition')],sort_keys=True)
    current={contract(p) for p in a.get('call_paths',[a.get('call_chain',[])])};native={contract(p) for p in b.get('call_paths',[b.get('call_chain',[])])}
    return current<=native

def attribute(current,control,game,mods):
    from stellar import children
    nodes=collections.defaultdict(list)
    relevant={d.get('definition') for d in current.diagnostics if d.get('definition')}
    for category,defs in control.definitions.items():
        for name,n in defs.items():
            if name not in relevant:continue
            def walk(x):
                nodes[digest(x)].append({'path':x.get('_source_path',n['_source_path']),'line':x.get('line',0),'column':x.get('column',0),'definition':name})
                for c in children(x):walk(c)
            walk(n)
    # Script definitions live in a separate database.
    for group in control.scripts.values():
        for name,(n,path) in group.items():
            if name not in relevant:continue
            def walk_script(x):
                nodes[digest(x)].append({'path':path,'line':x.get('line',0),'column':x.get('column',0),'definition':name})
                for c in children(x):walk_script(c)
            walk_script(n)
    native=collections.defaultdict(list)
    for d in control.diagnostics:native[(d.get('expression_sha256'),d['key'],d['message'],d['severity'])].append(d)
    definition_hashes=collections.defaultdict(set)
    for group in control.definitions.values():
        for name,n in group.items():
            if name in relevant:definition_hashes[name].add(digest(n))
    for group in control.scripts.values():
        for name,(n,path) in group.items():
            if name in relevant:definition_hashes[name].add(digest(n))
    for d in current.diagnostics:
        path=Path(d['path']);layer='game' if path.is_relative_to(game) else next(('dependency:'+str(i) for i,m in enumerate(mods[:-1]) if path.is_relative_to(m)),'mod' if mods and path.is_relative_to(mods[-1]) else 'external')
        candidates=[x for x in nodes.get(d.get('expression_sha256'),[]) if x.get('definition')==d.get('definition')]
        matches=native.get((d.get('expression_sha256'),d['key'],d['message'],d['severity']),[])
        matches=[x for x in matches if same_contract(d,x)]
        status='vanilla-existing' if matches else 'unconfirmed'
        # A corresponding definition with a changed statement is mod-added or
        # modified, not proof that the mod creates an engine bug.
        if not matches and not candidates and layer=='mod' and d.get('definition') is not None:status='mod-new-or-modified'
        elif candidates and not matches and d.get('call_chain'):
            # A missing diagnostic is not evidence of a missing caller. A
            # declaration-only modifier check has no script-call context.
            if any(f.get('kind')=='scripted-call' for path in d.get('call_paths',[d['call_chain']]) for f in path):status='new-call-context-unconfirmed'
        owner_hashes=definition_hashes.get(d.get('definition'),set())
        definition_modified=None if not owner_hashes else d.get('definition_sha256') not in owner_hashes
        positions=candidates or [{'path':x['path'],'line':x['line'],'column':x['column'],'definition':x.get('definition')} for x in matches]
        d['attribution']={'source_layer':layer,'classification':status,'statement_same_as_native':bool(positions),'containing_definition_modified':definition_modified,'native_positions':positions[:12],'control_diagnostic_positions':[{'path':x['path'],'line':x['line'],'column':x['column']} for x in matches],'native_control_scope':'only native counterpart target files and their reached script calls; not a complete vanilla semantic scan','engine_fault_proven':False}

def coverage_summary(checker):
    groups={}
    for d in checker.diagnostics:
        if d['key']!='validation-coverage':continue
        frame=next((f for f in d.get('call_chain',[]) if f.get('category')),None)
        module=frame['category'] if frame else ('graphics' if '/gfx/' in d['path'].replace('\\','/') else 'context')
        family=d['message'].split(':',1)[0];key=(module,family)
        item=groups.setdefault(key,{'module':module,'rule_family':family,'status':'partial-or-unsupported','count':0,'representative_positions':[],'affected_definitions':set(),'priority':'P1' if module in {'component_templates','ship_sizes','section_templates','global_ship_designs'} else 'P2'})
        item['count']+=1;item['affected_definitions'].add(d.get('definition') or '<unknown>')
        if len(item['representative_positions'])<3:item['representative_positions'].append({'path':d['path'],'line':d['line'],'message':d['message']})
    for g in groups.values():g['affected_definitions']=sorted(g['affected_definitions'])
    return sorted(groups.values(),key=lambda x:(x['priority'],-x['count'],x['module'],x['rule_family']))

def binding(checker,args):
    import sys
    from native_parser import parser_path
    implementation={'binary_sha256':hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest()} if getattr(sys,'frozen',False) else {'source_sha256':digest({p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in Path(__file__).parent.glob('*.py')})}
    return {'game':str(args.game.resolve()),'dependencies':[str(p.resolve()) for p in args.dependency],'target':str(args.mod.resolve()) if args.mod else None,'rules_sha256':hashlib.sha256(args.rules.read_bytes()).hexdigest(),'declarations_sha256':hashlib.sha256((__import__('native_parser').resource_root()/'rules/declarations-4.5.2.json').read_bytes()).hexdigest(),'parser_sha256':hashlib.sha256(parser_path().read_bytes()).hexdigest(),'input_set_sha256':digest(sorted((p['path'],p['sha256']) for p in checker.input_manifest)),'tool_version':__import__('stellar').VERSION,'implementation':implementation,'language':getattr(args,'language',None),'syntax_only':args.syntax_only}

def diagnostic_delta(report,previous):
    # Compare only identical physical inputs and hashes. Different rule versions
    # are recorded as the independent variable, never called input stability.
    old={(x['path'],x['sha256']) for x in previous.get('input_manifest',[])};new={(x['path'],x['sha256']) for x in report['input_manifest']}
    if old!=new:return {'comparable':False,'reason':'input paths or hashes differ','old_only':len(old-new),'new_only':len(new-old)}
    def sig(d):return (d['path'],d['line'],d['column'],d['key'],d['message'],d['severity'])
    a={sig(d) for d in previous['diagnostics']};b={sig(d) for d in report['diagnostics']}
    return {'comparable':True,'previous_tool_version':previous['tool_version'],'previous_rules_sha256':previous.get('rules_sha256'),'current_rules_sha256':report['rules_sha256'],'added':[list(x) for x in sorted(b-a)],'removed':[list(x) for x in sorted(a-b)],'unchanged':len(a&b)}
