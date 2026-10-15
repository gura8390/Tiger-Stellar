"""Opt-in project policy and visible overlays. Never changes player settings."""
import collections,difflib,hashlib,json
from pathlib import Path
from reporting import digest

def ai_possible(nodes,actor_path=()):
    """Evaluate with actor is_ai=yes. Unknown unrelated conditions stay unknown."""
    from stellar import children
    results=[]
    for n in nodes:
        k=n.get('key','').lower();value=n.get('value')
        if not actor_path and k=='is_ai':r=True if value=='yes' else False if value=='no' else None
        elif actor_path and k==actor_path[0]:r=ai_possible(children(n),actor_path[1:])
        elif k=='always':r=True if value=='yes' else False if value=='no' else None
        elif k in {'and','or','nor','not','nand','hidden_trigger','custom_tooltip','custom_tooltip_with_modifiers'}:
            parts=[ai_possible([c],actor_path) for c in children(n) if c.get('key') not in {'text','fail_text','success_text'}]
            if k in {'or','nor'}:r=True if True in parts else None if None in parts else False
            elif k in {'not','nor'}:r=False if False in parts else None if None in parts else True
            else:r=False if False in parts else None if None in parts else True
            if k in {'nor','not','nand'}:r=None if r is None else not r
        else:r=None
        results.append(r)
    return False if False in results else None if None in results else True

def constraints(checker,config):
    from stellar import children,fields
    allowed={'schema_version','player_only'}
    if set(config)-allowed or config.get('schema_version')!=1:raise ValueError('unsupported project constraint schema')
    result=[]
    for rule in config.get('player_only',[]):
        if set(rule)-{'category','definition','checks'}:raise ValueError('unknown project constraint parameter')
        category=rule['category'];name=rule['definition'];n=checker.definitions.get(category,{}).get(name)
        if not n:
            checker.emit('project-constraint','configured definition is missing: '+category+'/'+name);continue
        checker.file=n['_source_path']
        for check in rule['checks']:
            if set(check)-{'purpose','field_path','actor_path'}:raise ValueError('unknown project constraint check parameter')
            path=check['field_path'];actor=tuple(check.get('actor_path',[]));nodes=[n]
            # Repeated produces/triggered modifiers are separate branches.
            # Every configured branch must exclude AI; last-field lookup is not
            # a proof for the earlier branches.
            for part in path:nodes=[c for parent in nodes for c in children(parent) if c.get('key')==part]
            outcomes=[ai_possible(children(node),actor) for node in nodes] or [True]
            feasible=True if True in outcomes else None if None in outcomes else False
            node=next((node for node,outcome in zip(nodes,outcomes) if outcome is not False),nodes[0] if nodes else n)
            status='AI excluded by implemented boolean proof' if feasible is False else 'AI path possible in this condition' if feasible is True else 'unknown'
            record={'category':category,'definition':name,'purpose':check['purpose'],'field_path':path,'actor_path':list(actor),'status':status,'branches_checked':len(nodes),'branch_results':outcomes,'runtime_ownership_validated':False};result.append(record)
            if feasible is True:checker.emit('project-constraint',check['purpose']+': no effective AI exclusion at '+'.'.join(path),node or n)
            elif feasible is None:checker.gap('project player-only proof unresolved: '+name+'/'+check['purpose'],node or n)
    return result

def overlay(checker,game,mods):
    from stellar import collect_files,parse_text
    records={};layers=[game]+mods
    for i,root in enumerate(layers):
        descriptor=root/'descriptor.mod'
        prefixes=[]
        if i and descriptor.exists():
            prefixes=[n.get('value','').strip('/').lower()+'/' for n in parse_text(descriptor.read_text(encoding='utf-8-sig'),str(descriptor))['ast'] if n.get('key')=='replace_path']
        for rel,record in records.items():
            if any(rel.startswith(p) for p in prefixes):
                record['removed_by_replace_path'].append({'layer':i,'root':str(root)});record['effective']=None
        for rel,p in collect_files(root).items():
            record=records.setdefault(rel,{'relative_path':rel,'providers':[],'removed_by_replace_path':[],'effective':None})
            provider={'path':str(p),'layer':i,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()};record['providers'].append(provider);record['effective']=provider
            checker.asset_inputs[str(p)]={'relative_path':rel,'path':str(p),'sha256':provider['sha256'],'overlay_source':True}
    files=[]
    for rel,r in sorted(records.items()):
        if len(r['providers'])>1:
            a=Path(r['providers'][-2]['path']).read_text(encoding='utf-8-sig',errors='replace').splitlines();b=Path(r['providers'][-1]['path']).read_text(encoding='utf-8-sig',errors='replace').splitlines()
            delta=list(difflib.unified_diff(a,b,fromfile=r['providers'][-2]['path'],tofile=r['providers'][-1]['path'],lineterm=''))
            r['content_diff']={'added_lines':sum(x.startswith('+') and not x.startswith('+++') for x in delta),'removed_lines':sum(x.startswith('-') and not x.startswith('---') for x in delta),'unified_diff':delta}
        files.append(r)
    effective=[]
    for cat,defs in checker.definitions.items():
        for name,n in defs.items():effective.append({'category':cat,'name':name,'provider':n.get('_source_path'),'line':n.get('line',0),'semantic_sha256':digest(n),'previous_surviving_definitions':checker.definition_history.get((cat,name),[])[:-1]})
    for kind,defs in checker.scripts.items():
        for name,(n,p) in defs.items():effective.append({'category':'scripted_'+kind,'name':name,'provider':p,'line':n.get('line',0),'semantic_sha256':digest(n)})
    return {'layers':[{'index':i,'root':str(p)} for i,p in enumerate(layers)],'resolution':'checker lexical file order after file and replace_path overlay; later definition wins; not runtime load verification','files':files,'effective_definitions':effective,'same_name_conflicts':[x for x in effective if x.get('previous_surviving_definitions')]}
