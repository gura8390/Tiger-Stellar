"""Name-based entries backed by installed source comments, not guessed names."""
import hashlib,re
from pathlib import Path

class EngineEntries:
    def prepare_engine_entries(self,game):
        specs={
          'planet_killer_trigger':('common/scripted_triggers/01_scripted_triggers_planet_killers.txt', ['this = planet being targeted','from = fleet']),
          'planet_killer_action':('common/on_actions/01_planet_destruction.txt',['this/root = planet that have been fired upon','from = fleet that fired'])}
        self.entry_sources={}
        self.documented_actions={}
        for name,(rel,markers) in specs.items():
            p=game/rel
            if not p.is_file():continue
            text=p.read_text(encoding='utf-8-sig')
            if not all(x in text for x in markers):continue
            evidence={'relative_path':rel,'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'evidence_kind':'installed vanilla documentation comment','markers':markers}
            self.entry_sources[name]=evidence
        # Only explicit, immediately preceding vanilla scope annotations count.
        # Missing/ambiguous annotations remain unknown; suffixes are not contracts.
        from stellar_scopes import ENGINE_SCOPE_TYPES
        aliases={'system':'galactic_object','solar system':'galactic_object','astral rift':'astral_rift','pop group':'pop_group','no scope':'none'}
        for p in sorted((game/'common/on_actions').glob('*.txt')):
            text=p.read_text(encoding='utf-8-sig');lines=text.splitlines()
            for index,line in enumerate(lines):
                match=re.match(r'^(on_\w+)\s*=\s*\{',line)
                if not match:continue
                comments=[];cursor=index-1
                while cursor>=0 and (not lines[cursor].strip() or lines[cursor].lstrip().startswith('#')):
                    comments.insert(0,lines[cursor]);cursor-=1
                documented={};optional=[];markers=[]
                for comment in comments:
                    annotation=re.match(r'\s*#\s*(this/root|this|root|scope|(?:from)+)\s*=\s*(.+)',comment,re.I)
                    if not annotation:continue
                    label=annotation[1].lower();phrase=annotation[2].lower().strip()
                    if re.search(r'\bor\b',phrase):continue
                    name=re.split(r'[,.(]',phrase,1)[0].strip()
                    # Longer explanations must still start with a complete type.
                    scope=next((value for term,value in sorted(aliases.items(),key=lambda x:-len(x[0])) if name==term or name.startswith(term+' ')),None)
                    if scope is None:
                        word=name.split()[0] if name else ''
                        if word in ENGINE_SCOPE_TYPES-{'all','various','no_scope'}:scope=word
                    if scope is None:continue
                    labels=['this','root'] if label=='this/root' else ['this' if label=='scope' else label]
                    for key in labels:documented[key]=scope
                    if any(term in phrase for term in ['if any','if available','optional']):optional.extend(labels)
                    markers.append(comment)
                if not ({'this','root'} & documented.keys()):continue
                evidence={'relative_path':p.relative_to(game).as_posix(),'path':str(p),'line':index+1,'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'evidence_kind':'installed vanilla documentation comment','markers':markers}
                self.documented_actions[match[1]]={'scopes':documented,'optional':optional,'evidence':evidence}

    def engine_contract(self,name,kind,node):
        from stellar import Context,UNKNOWN,fields
        documented=getattr(self,'documented_actions',{}).get(name) if kind=='on_actions' else None
        if documented:
            scopes=documented['scopes'];current=scopes.get('this',scopes.get('root',UNKNOWN));root=scopes.get('root',UNKNOWN)
            depth=max((key.count('from') for key in scopes if key.startswith('from')),default=0)
            ctx=Context(current,root,tuple(scopes.get('from'*i,UNKNOWN) for i in range(1,depth+1)))
            if not self.inference_mode:
                record={'name':name,'kind':kind,'source':self.file,'line':node.get('line',0),'context':{'this':ctx.current,'root':ctx.root,'froms':list(ctx.froms),'prevs':[]},'optional_scopes':documented['optional'],'registration':'explicit vanilla on_action annotation','ordinary_script_call_proven':False,'runtime_invocation_proven':False,'runtime_target_validity_proven':False,'evidence':documented['evidence']}
                if record not in self.engine_entries:self.engine_entries.append(record)
            return ctx
        trigger=kind=='triggers' and name.startswith('can_destroy_planet_with_')
        action=kind=='on_actions' and name.startswith('on_destroy_planet_with_')
        if not trigger and not action:return None
        component=name.split('_with_',1)[1]
        if action:
            for suffix in ['_unqueued','_queued']:
                if component.endswith(suffix):component=component[:-len(suffix)];break
        definition=self.definitions.get('component_templates',{}).get(component)
        source=getattr(self,'entry_sources',{}).get('planet_killer_trigger' if trigger else 'planet_killer_action')
        if not definition or fields(definition).get('type',{}).get('value')!='planet_killer' or not source:return None
        ctx=Context('planet',UNKNOWN if trigger else 'planet',('fleet',))
        if not self.inference_mode:
            record={'name':name,'kind':kind,'component':component,'source':self.file,'line':node.get('line',0),'context':{'this':ctx.current,'root':ctx.root,'froms':list(ctx.froms),'prevs':[]},'registration':'component and naming contract matched','ordinary_script_call_proven':False,'runtime_invocation_proven':False,'runtime_target_validity_proven':False,'evidence':source}
            if record not in self.engine_entries:self.engine_entries.append(record)
        return ctx
