"""Name-based entries backed by installed source comments, not guessed names."""
import hashlib
from pathlib import Path

class EngineEntries:
    def prepare_engine_entries(self,game):
        specs={
          'planet_killer_trigger':('common/scripted_triggers/01_scripted_triggers_planet_killers.txt', ['this = planet being targeted','from = fleet']),
          'planet_killer_action':('common/on_actions/01_planet_destruction.txt',['this/root = planet that have been fired upon','from = fleet that fired'])}
        self.entry_sources={}
        for name,(rel,markers) in specs.items():
            p=game/rel
            if not p.is_file():continue
            text=p.read_text(encoding='utf-8-sig')
            if not all(x in text for x in markers):continue
            evidence={'relative_path':rel,'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'evidence_kind':'installed vanilla documentation comment','markers':markers}
            self.entry_sources[name]=evidence

    def engine_contract(self,name,kind,node):
        from stellar import Context,UNKNOWN,fields
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
