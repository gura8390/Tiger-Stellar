"""Tiger Stellar: engine-derived API, typed scope traversal, layered entity databases.

GPL-3.0-or-later. Syntax is parsed by Tiger's native LALRPOP parser.
Unknown semantic regions are diagnostics, never implicit validation successes.
"""
import argparse, collections, copy, hashlib, json, re, sys, time, uuid, math
from dataclasses import dataclass, replace
from pathlib import Path
from native_parser import parse_many, parse_text, resource_root, parser_path, explain_braces
from templates import render
from stellar_scopes import ENGINE_SCOPE_TYPES
from coverage_rules import CoverageRules

# Mixins import the public module at call time. The executable's entry point
# must refer to this same module and Context type, not a second imported copy.
if __name__=='__main__':sys.modules['stellar']=sys.modules[__name__]

VERSION='0.1.7'
UNKNOWN='?'
def children(n): return n.get('children',[])
def fields(n): return {x.get('key'):x for x in children(n) if 'key' in x}
def values(n): return [x['value'] for x in children(n) if 'value' in x and 'key' not in x]
def number(v):
    try: return float(v)
    except (ValueError,TypeError): return None

def scoped_numeric_variable(value):
    """Dots inside reader expressions/parameter values aren't scope separators.

    Explicit expression syntaxes have their own parsers below. Only a dotted
    variable reference reaches resolve(), where misspelled links still error.
    """
    return bool(value and '.' in value and number(value) is None and not value.startswith('@') and not any(marker in value for marker in ['value:','modifier:','trigger:']))

@dataclass(frozen=True)
class Context:
    current: str
    root: str
    froms: tuple=()
    prevs: tuple=()
    def push(self,scope): return replace(self,current=scope,prevs=(self.current,)+self.prevs)

EVENT_SCOPES={'country_event':'country','planet_event':'planet','ship_event':'ship','fleet_event':'fleet','pop_event':'pop','pop_group_event':'pop_group','leader_event':'leader','starbase_event':'starbase','system_event':'galactic_object','situation_event':'situation','colony_event':'colony','carrier_event':'planet|ship','event':'none'}
EVENT_SCOPES['observer_event']='country'
EVENT_SCOPES.update({'astral_rift_event':'astral_rift','first_contact_event':'first_contact','espionage_operation_event':'espionage_operation','agreement_event':'agreement','bypass_event':'bypass','pop_faction_event':'pop_faction'})
REFS={'has_technology':'technology','give_technology':'technology','has_building':'buildings','add_building':'buildings','remove_building':'buildings','has_trait':'traits','add_trait':'traits','remove_trait':'traits','is_planet_class':'planet_classes','change_pc':'planet_classes','has_ethic':'ethics','has_civic':'governments/civics','has_authority':'governments/authorities','has_origin':'governments/civics','has_ascension_perk':'ascension_perks','has_tradition':'traditions','has_valid_civic':'governments/civics','remove_modifier':'static_modifiers','has_modifier':'static_modifiers','has_deposit':'deposits','add_deposit':'deposits','remove_deposit':'deposits','is_ship_size':'ship_sizes','is_megastructure_type':'megastructures'}
TARGET_COUNTRY={'is_owned_by','is_controlled_by','is_same_empire','set_owner','set_controller','set_space_owner','has_communications','has_truce','is_at_war_with','is_in_federation_with','is_subject_of','is_overlord_to','has_defensive_pact','has_non_aggression_pact','has_migration_access','has_commercial_pact'}
REFS['has_government']='governments'
REFS['has_component']='component_templates'
REFS.update({'has_district':'districts','has_building_construction':'buildings'})
REFS['has_psionic_aura']='patrons/psionic_auras'
REFS['has_resource']='strategic_resources'
LOGIC={'and','or','not','nor','nand','hidden_trigger','hidden_effect','hidden_tooltip','tooltip','custom_tooltip','custom_tooltip_fail','success_text','custom_tooltip_with_modifiers','custom_description','if','else_if','else','while'}

class Checker(CoverageRules):
    def __init__(self,rules):
        if rules.get('game_id')!='stellaris':raise ValueError('wrong-game: Tiger Stellar requires Stellaris rules')
        for kind in ['triggers','effects','scopes','numeric_expressions']:
            for name,rule in rules.get(kind,{}).items():
                actual=set(rule['input'])|set(filter(None,rule.get('output','').split('|')))|set(filter(None,rule.get('iterator_output','').split('|')))
                for source,target in rule.get('output_by_input',{}).items():actual.add(source);actual.update(target.split('|'))
                if actual-ENGINE_SCOPE_TYPES:raise ValueError('wrong-game: non-Stellaris scope types in '+kind+'/'+name+': '+', '.join(sorted(actual-ENGINE_SCOPE_TYPES)))
        self.rules=copy.deepcopy(rules); self.diagnostics=[]; self.entities=collections.defaultdict(set)
        self.scripts={'triggers':{},'effects':{}}; self.events={};self.event_defs={};self.loc=set();self.targets={}
        self.checked=collections.Counter();self.gaps=collections.Counter();self._unique=set();self.active=[]
        self._diagnostic_by_signature={}
        self.global_targets={};self.global_unknown_targets=set();self.inference_mode=False;self.inferred_script_inputs={}
        self.asset_roots=[];self.asset_inputs={};self.variable_types={};self.project_locations={};self.reader_variables={}
        self.definitions=collections.defaultdict(dict);self.weapon_stats={};self.engine_entries=[];self.ship_feasibility=[]
        self.definition_history=collections.defaultdict(list)
        self.event_contexts={};self.next_event_contexts={};self.script_contexts={}
        self.modifiers=set(rules['modifiers']);self.file='';self.mod_files=set();self.input_manifest=[];self.inline={};self.globals_text='';self.invoked=set();self.expansion_trace=[]
        decl=resource_root()/'rules/declarations-4.5.2.json'
        declarations=json.loads(decl.read_text(encoding='utf8'))
        if declarations.get('game_id')!='stellaris':raise ValueError('wrong-game: Tiger Stellar requires Stellaris declaration rules')
        self.declarations=declarations['categories']
    def emit(self,key,message,node=None,severity='error',path=None):
        if self.inference_mode:return
        node=node or {};path=path or node.get('_source_path') or self.file
        signature=(path,node.get('line',0),node.get('column',0),key,message,severity)
        if signature in self._unique:
            from reporting import annotate
            record=self._diagnostic_by_signature.get(signature)
            if record is not None:
                snapshot={'severity':severity,'key':key};annotate(self,node,snapshot)
                if snapshot['call_chain'] not in record['call_paths']:record['call_paths'].append(snapshot['call_chain'])
            return record
        self._unique.add(signature)
        self.diagnostics.append(dict(path=path,line=node.get('line',0),column=node.get('column',0),key=key,severity=severity,message=message))
        from reporting import annotate
        annotate(self,node,self.diagnostics[-1])
        self.diagnostics[-1]['call_paths']=[self.diagnostics[-1]['call_chain']]
        self._diagnostic_by_signature[signature]=self.diagnostics[-1]
        from diagnostic_kinds import classify
        classify(self.diagnostics[-1])
        return self.diagnostics[-1]
    def gap(self,message,node=None,reason=None,evidence=None):
        if self.inference_mode:return
        self.gaps[message]+=1;record=self.emit('validation-coverage',message,node,'warning')
        if reason or evidence:
            from diagnostic_kinds import classify
            classify(record,reason,evidence)
    def require_block(self,n):
        if 'children' not in n:self.emit('structure','expected a block for '+n.get('key','item'),n);return False
        return True
    def compatible(self,scope,accepted,n,label):
        if 'all' in accepted:return True
        if scope==UNKNOWN:
            self.gap('scope cannot be inferred for '+label,n,'scope-type-unknown',{'operation':label,'accepted_scopes':sorted(accepted),'actual_scope':scope,'statement':{k:n[k] for k in ['key','value'] if k in n},'runtime_existence_proven':False});return True
        candidates=set(scope.split('|'))
        if candidates & set(accepted) and not candidates <= set(accepted):
            self.gap('polymorphic scope '+scope+' for '+label,n,'scope-type-ambiguous',{'operation':label,'accepted_scopes':sorted(accepted),'candidate_scopes':sorted(candidates),'accepted_candidates':sorted(candidates & set(accepted)),'unproven_or_incompatible_candidates':sorted(candidates-set(accepted))});return True
        if not candidates & set(accepted):
            self.emit('scope',f'{label} accepts {", ".join(accepted)}; current scope is {scope}',n);return False
        return True
    def resolve(self,path,ctx,n):
        if path.startswith(('event_target:','global_event_target:')):
            if '@' in path:
                # The suffix selects a per-object target name, not a scope
                # transition applied to the saved object's type. A keyed
                # lookup cannot inherit facts from an unkeyed target.
                name,selector=path.split('@',1)
                self.resolve(selector,ctx,n)
                self.gap('dynamic event target identity cannot be inferred: '+name,n)
                return ctx.push(UNKNOWN)
            head,_,tail=path.partition('.')
            alias='global_event_target:'+head.split(':',1)[1]
            global_type=UNKNOWN if alias in self.global_unknown_targets else self.global_targets.get(alias,UNKNOWN)
            result=ctx.push(self.targets.get(head,self.targets.get(alias,global_type)))
            return self.resolve(tail,result,n) if tail else result
        current=ctx
        for part in path.rstrip('?').lower().split('.'):
            if part=='root':scope=ctx.root
            elif part=='this':scope=current.current
            elif re.fullmatch(r'(from)+',part):
                index=part.count('from')-1;scope=ctx.froms[index] if index<len(ctx.froms) else UNKNOWN
            elif re.fullmatch(r'(prev)+',part):
                index=part.count('prev')-1
                if index>=len(current.prevs):self.emit('scope','previous scope does not exist: '+part,n);scope=UNKNOWN
                else:scope=current.prevs[index]
            elif part in self.rules['scopes']:
                rule=self.rules['scopes'][part];self.compatible(current.current,rule['input'],n,part)
                outputs={rule.get('output_by_input',{}).get(source,rule['output']) for source in current.current.split('|')}
                scope=UNKNOWN if outputs & {'various','all',UNKNOWN} else '|'.join(sorted(outputs))
            else:self.emit('unknown-scope','unknown scope link '+part,n);return ctx.push(UNKNOWN)
            current=current.push(scope)
        return current
    def is_scope(self,k):
        return k.rstrip('?').lower().split('.')[0] in self.rules['scopes'] or k.startswith(('event_target:','global_event_target:'))
    def scalar(self,n):
        if 'children' in n:self.emit('structure','expected scalar value for '+n.get('key','item'),n);return None
        return n.get('value','')
    def numeric(self,n,ctx=None):
        v=self.scalar(n)
        if v is None:return
        if number(v) is not None and not math.isfinite(number(v)):
            self.emit('value','numeric values must be finite: '+str(v),n);return
        if number(v) is None:
            # Script values can be evaluated in another scope. Parse the typed
            # expression BEFORE splitting any dots in its parameter values.
            if 'value:' in v or 'modifier:' in v:
                marker='value:' if 'value:' in v else 'modifier:'
                prefix,_,name=v.partition(marker)
                if prefix and not prefix.endswith('.'):
                    self.emit('value','numeric expression scope prefix must end with a dot: '+v,n);return
                if prefix:
                    if ctx is None:self.gap('numeric expression context cannot be inferred: '+v,n)
                    else:self.resolve(prefix[:-1],ctx,n)
                if marker=='value:':self.ref('script_values',name,n)
                elif name not in self.modifiers:self.emit('unknown-modifier','unknown numeric modifier '+name,n)
                return
            symbol=v.removeprefix('var:')
            target=ctx
            if ctx and scoped_numeric_variable(symbol):
                prefix,_,symbol=symbol.rpartition('.')
                target=self.resolve(prefix,ctx,n)
            known={self.variable_types.get((scope,symbol)) for scope in (target.current.split('|') if target else [])}
            if known=={'number'}:return
            if v.startswith('modifier:'):
                if v[9:] not in self.modifiers:self.emit('unknown-modifier','unknown numeric modifier '+v[9:],n)
                return
            if 'trigger:' in v:
                prefix,_,name=v.rpartition('trigger:')
                rule=self.rules['triggers'].get(name) or self.rules.get('numeric_expressions',{}).get(name)
                if not rule:self.emit('unknown-trigger','unknown numeric trigger '+name,n);return
                if ctx is None:self.gap('numeric expression context cannot be inferred: '+v,n);return
                target=self.resolve(prefix.rstrip('.'),ctx,n) if prefix else ctx
                self.compatible(target.current,rule['input'],n,'numeric '+name)
                if rule.get('boolean'):self.emit('value','numeric expression requires a number, got boolean '+name,n)
                elif not rule.get('numeric'):self.gap('numeric trigger result type is not documented: '+name,n)
            elif v.startswith('value:'):self.ref('script_values',v[6:],n)
            elif v.startswith(('var:','trigger:','modifier:','@')) or '.' in v: self.gap('numeric expression not evaluated: '+v,n)
            elif re.fullmatch(r'[A-Za-z_]\w*',v):self.gap('numeric variable cannot be resolved statically: '+v,n)
            else:self.emit('value','expected numeric value, script value or variable: '+v,n)
    def numeric_range(self,n):
        if 'children' not in n:self.numeric(n);return
        for child in children(n):
            if child.get('key') not in {'min','max'}:self.emit('unknown-field','numeric range expects min/max',child)
            else:self.numeric(child)
    def order_expression(self,n,ctx):
        value=self.scalar(n)
        if value is None:return
        if not value.startswith('trigger:'):self.numeric(n);return
        name=value[len('trigger:'):];rule=self.rules['triggers'].get(name) or self.rules.get('numeric_expressions',{}).get(name)
        if not rule:self.emit('unknown-trigger','unknown ordering trigger '+name,n);return
        self.compatible(ctx.current,rule['input'],n,'order_by '+name)
        if rule.get('boolean'):self.emit('value','order_by requires a numeric trigger, got '+name,n)
        elif not rule.get('numeric') and 'iterator_output' not in rule:self.gap('ordering trigger result type is not documented: '+name,n)
    def ref(self,category,v,n):
        if not v or v in {'yes','no','random','none'} or (category=='governments' and v=='any'):return
        if v.startswith(('@','$','event_target:','value:','var:')) or re.match(r'^(?:root|this|(?:from)+|(?:prev)+)(?:\.|$)',v):
            self.gap('dynamic entity reference cannot be resolved: '+v,n);return
        if category=='script_values' and '|' in v:
            head,*parts=v.split('|')
            if parts and parts[-1]=='':parts.pop()
            if len(parts)%2:self.emit('value','script value arguments require NAME|VALUE pairs',n);return
            if head not in self.entities[category]:self.emit('missing-item',f'unknown {category} reference `{head}`',n);return
            for key,value in zip(parts[::2],parts[1::2]):
                if not re.fullmatch(r'\w+',key) or not value:self.emit('value','invalid script value parameter '+key,n)
                elif key=='MODIFIER' and value not in self.modifiers:self.emit('unknown-modifier','unknown modifier '+value,n)
            self.gap('parameterized script value result is not evaluated: '+head,n);return
        if v not in self.entities[category]:self.emit('missing-item',f'unknown {category} reference `{v}`',n)
    def localisation(self,n):
        v=n.get('value','')
        language=getattr(self,'language',None);keys=getattr(self,'loc_languages',{}).get(language,set()) if language else self.loc
        if v and not any(c in v for c in ' []$') and v not in {'default','none','yes','no'} and v not in keys:
            self.emit('localization',f'localization key `{v}` was not found in '+(language if language else 'any loaded language'),n,'warning')
    def expand(self,k,n,kind,ctx):
        definition,path=self.scripts[kind][k]
        self.remember_script_context(kind,k,n,ctx)
        self.invoked.add((kind,k));self.expansion_trace.append({'caller':self.file,'line':n.get('line',0),'definition':path,'name':k,'scope':ctx.current,'root':ctx.root,'froms':list(ctx.froms),'prevs':list(ctx.prevs)})
        if k in self.active or len(self.active)>40:self.gap('recursive scripted call: '+k,n);return
        if 'macro_parts' in definition:
            args={x.get('key'):x.get('value') for x in children(n)}
            parameters={p.split('|',1)[0]:p.partition('|')[2] if '|' in p else None for p in definition['macro_parameters']}
            conditional_names=set(re.findall(r'\[\[!?([\w]+)\]', ''.join(p['text'] for p in definition['macro_parts'])))
            missing={p for p,default in parameters.items() if p not in args and default is None and p not in conditional_names}
            extra=args.keys()-(parameters.keys()|conditional_names)
            if extra:self.emit('macro-argument','unexpected macro arguments: '+', '.join(sorted(extra)),n)
            if any(v is None for v in args.values()):self.gap('block-valued macro argument: '+k,n);return
            for p,default in parameters.items():
                if p not in args and default is not None:args[p]=default
            source=''.join('$'+p['text']+'$' if p['parameter'] else p['text'] for p in definition['macro_parts'])
            try:text=render(source,args)
            except ValueError as e:self.emit('macro-argument',str(e),n);return
            local='\n'.join('@'+name+' = '+str(value) for name,value in definition.get('macro_variables',{}).items())
            parsed=parse_text(local+'\nexpanded = {'+text+'}',path,self.globals_text)
            for d in parsed['diagnostics']:self.emit(d['key'],d['message'],n,d['severity'])
            body=children(parsed['ast'][0]) if parsed['ast'] else []
            shift=definition.get('line',1)-(local+'\n').count('\n')-1
            def relocate(items):
                for item in items:
                    item['line']=item.get('line',1)+shift
                    relocate(children(item))
            relocate(body)
            def conditionals(items):
                result=[]
                for item in items:
                    key=item.get('key','')
                    if key.startswith('@conditional:'):
                        cond=key.split(':',1)[1];negative=cond.startswith('!');present=cond.lstrip('!') in args
                        if present != negative:result.extend(conditionals(children(item)))
                    else:
                        item=copy.deepcopy(item)
                        if 'children' in item:item['children']=conditionals(item['children'])
                        result.append(item)
                return result
            body=conditionals(body)
        else:
            if 'children' in n:self.emit('structure','scripted call takes yes/no, not arguments: '+k,n)
            elif n.get('value') not in {'yes','no'}:
                bodykeys=[i.get('key','') for i in children(definition)]
                if not any(key.startswith('count_') or key in {'add','subtract','multiply','divide'} for key in bodykeys):self.emit('value','scripted boolean call expects yes/no: '+k,n)
            body=children(definition)
        self.active.append(k);saved=self.file;self.file=path
        self.script(body,kind,ctx)
        self.file=saved;self.active.pop()
    def script(self,body,kind,ctx):
        false_ctx=ctx
        for n in body:
            if 'key' not in n:
                if n.get('value')=='optimize_memory':continue
                self.emit('structure','expected script statement',n);continue
            k=n['key'];low=k.lower();v=n.get('value');self.checked[kind]+=1
            if low.startswith('hidden:'):
                self.script([{**n,'key':k[7:]}],kind,ctx);continue
            if k=='inline_script':self.expand_inline(n,kind,ctx);continue
            if k.startswith('@'):continue
            if k in self.scripts[kind] and low not in LOGIC and k not in self.rules[kind]:
                if self.inference_mode and hasattr(self,'inference_interesting') and k not in self.inference_interesting:continue
                self.expand(k,n,kind,ctx);continue
            if self.is_scope(k):
                if self.require_block(n):self.script(children(n),kind,self.resolve(k,ctx,n))
                continue
            if low in LOGIC:
                if low in {'custom_tooltip','custom_tooltip_with_modifiers'} and v is not None:self.localisation(n);continue
                if not self.require_block(n):continue
                nested=ctx
                if low in {'and','hidden_trigger','custom_tooltip','custom_tooltip_with_modifiers'}:
                    nested=self.narrow(children(n),ctx)
                elif low in {'if','else_if','else'}:
                    base=ctx if low=='if' else false_ctx
                    limit=children(fields(n).get('limit',{}))
                    if low!='else':
                        if base:self.script(limit,'triggers',base)
                        nested=self.narrow(limit,base) if base else None
                        false_ctx=self.narrow(limit,base,False) if base else None
                    else:nested=base
                if nested is None:continue
                statements=[]
                for child in children(n):
                    if child.get('key') in {'text','fail_text','success_text','tooltip','desc','localization_key'} and 'value' in child:self.localisation(child)
                    elif child.get('key')=='limit':
                        if low not in {'if','else_if'}:self.script(children(child),'triggers',ctx)
                    elif low=='while' and child.get('key')=='count':self.numeric(child,ctx)
                    else:statements.append(child)
                self.script(statements,kind,nested)
                continue
            if low=='limit':
                if self.require_block(n):self.script(children(n),'triggers',ctx)
                continue
            if low=='switch':
                f=fields(n);trigger=f.get('trigger',{}).get('value')
                if trigger and trigger not in self.rules['triggers'] and trigger not in self.scripts['triggers']:self.emit('unknown-trigger','unknown switch trigger '+trigger,n)
                elif trigger in self.rules['triggers']:self.compatible(ctx.current,self.rules['triggers'][trigger]['input'],f.get('trigger',n),trigger)
                for child in children(n):
                    if child.get('key')=='inline_script':
                        self.expand_inline(child,'switch-cases',ctx,handler=lambda body:self.script([{'key':'switch','children':[f['trigger']]+children(body)}],kind,ctx))
                    elif child.get('key') not in {'trigger','on_trigger'}:self.script(children(child),kind,ctx)
                continue
            if low in {'random_list','locked_random_list'}:
                for branch in children(n):
                    if number(branch.get('key')) is None:self.gap('random_list metadata '+branch.get('key',''),branch);continue
                    self.weighted(branch,kind,ctx)
                continue
            if kind=='effects' and low=='random':
                if self.require_block(n):
                    for child in children(n):
                        if child.get('key')=='chance':self.numeric(child)
                        else:self.script([child],kind,ctx)
                continue
            if low=='custom_gui':
                # Event options refer to a named interface container, not script.
                value=self.scalar(n)
                if value is not None:self.ref('gui_containers',value,n)
                continue
            rule=self.rules[kind].get(k)
            if not rule:
                self.emit('unknown-'+('trigger' if kind=='triggers' else 'effect'),f'unknown {kind} token `{k}`',n);continue
            self.compatible(ctx.current,rule['input'],n,k)
            if k=='is_scope_type' and v not in ENGINE_SCOPE_TYPES|{'none','observer','alliance'}:self.emit('value','unknown Stellaris scope type '+str(v),n)
            if 'iterator_output' in rule:
                if self.require_block(n):
                    nested=ctx.push(rule['iterator_output'])
                    statements=[]
                    for child in children(n):
                        key=child.get('key')
                        if key=='order_by':self.order_expression(child,nested);continue
                        if key=='count' and child.get('value')=='all':continue
                        if key in {'count','percent','position','max_steps','min_steps'}:
                            self.numeric(child);continue
                        if key=='use_bypasses' or (key=='inverse' and k.startswith('ordered_')):
                            if self.scalar(child) not in {'yes','no'}:self.emit('value',key+' expects yes/no',child)
                            continue
                        if child.get('key')=='weights':self.weighted(child,'triggers',nested);continue
                        if child.get('key')=='limit':self.script(children(child),'triggers',nested)
                        else:statements.append(child)
                    self.script(statements,kind,nested)
                continue
            if k in {'save_event_target_as','save_global_event_target_as'} and v:
                target_name=('global_event_target:' if k.startswith('save_global') else 'event_target:')+v
                previous=self.targets.get(target_name,ctx.current)
                self.targets[target_name]='|'.join(sorted(set(previous.split('|'))|set(ctx.current.split('|'))))
                if k=='save_global_event_target_as' and UNKNOWN not in ctx.current.split('|'):
                    known=self.global_targets.get(target_name,ctx.current)
                    self.global_targets[target_name]='|'.join(sorted(set(known.split('|'))|set(ctx.current.split('|'))))
                elif k=='save_global_event_target_as':self.global_unknown_targets.add(target_name)
            if v is not None:
                if k in {'set_variable','change_variable','check_variable','create_ship','create_fleet','create_megastructure','add_modifier','country_event','planet_event','ship_event','fleet_event','carrier_event'}:
                    self.emit('structure',k+' requires a parameter block',n);continue
                if rule.get('boolean') and k not in set(REFS)|{'has_deposit','has_building','has_starbase_building','has_starbase_module'} and v not in {'yes','no'}:self.emit('value',k+' expects yes/no',n)
                if rule.get('numeric'):self.numeric(n,ctx)
                if k in REFS:
                    reserved=(k=='add_deposit' and v in {'random','random_blocker','random_nonblocker'}) or (k=='change_pc' and v=='owner_main_species')
                    if not reserved:self.ref(REFS[k],v,n)
                if k in TARGET_COUNTRY:
                    target=self.resolve(v,ctx,n);self.compatible(target.current,['country'],n,k+' target')
                elif k in {'exists','is_same_value'}:self.resolve(v,ctx,n)
            else:
                if rule.get('boolean') and k!='end_fleet_contract' and not re.search(r'\b'+re.escape(k)+r'\s*=\s*\{',rule['description']):self.emit('structure',k+' expects a scalar',n)
                else:self.parameters(k,n,kind,ctx,rule)
    def weighted(self,n,kind,ctx):
        for child in children(n):
            k=child.get('key')
            if k=='inline_script':self.expand_inline(child,'weight',ctx,handler=lambda body:self.weighted(body,kind,ctx))
            elif k=='complex_trigger_modifier':
                f=fields(child);trigger=f.get('trigger',{}).get('value');rule=self.rules['triggers'].get(trigger)
                for key,item in f.items():
                    if key not in {'trigger','trigger_scope','parameters','potential','mode','mult','add','factor','base'}:self.emit('unknown-field','complex_trigger_modifier: unknown field '+str(key),item)
                    elif key in {'mult','add','factor','base'}:self.numeric(item)
                    elif key=='potential':
                        if self.require_block(item):self.script(children(item),'triggers',ctx)
                if not rule:self.emit('unknown-trigger','unknown complex trigger '+str(trigger),f.get('trigger',child))
                else:
                    target=ctx
                    if 'trigger_scope' in f:
                        scope=self.scalar(f['trigger_scope'])
                        if scope is not None:target=self.resolve(scope,ctx,f['trigger_scope'])
                    self.compatible(target.current,rule['input'],f.get('trigger',child),trigger)
                    if 'iterator_output' in rule:self.script([{'key':trigger,'children':children(f.get('parameters',{}))}],'triggers',target)
                    elif 'parameters' in f:self.parameters(trigger,f['parameters'],'triggers',target,rule)
                if f.get('mode',{}).get('value','add') not in {'add','subtract','multiply','divide','mult','min','max','set','factor'}:self.emit('value','invalid complex trigger weighting mode',f['mode'])
            elif k in {'factor','weight','base','add','mult','subtract','multiply','divide','min','max','set','days','months','years'}:self.numeric(child)
            elif k=='desc':self.localisation(child)
            elif k=='modifier':self.weighted(child,'triggers',ctx)
            elif k=='limit':self.script(children(child),'triggers',ctx)
            else:self.script([child],kind,ctx)
    def expand_inline(self,n,kind,ctx,category=None,handler=None):
        f=fields(n);name=n.get('value') or f.get('script',{}).get('value')
        if not name:self.emit('missing-field','inline_script requires script path',n);return
        name=name.replace('\\','/').lower()
        if name not in self.inline:self.emit('missing-inline-script','unknown inline script '+name,n);return
        if 'inline:'+name in self.active:self.gap('recursive inline script '+name,n);return
        path=self.inline[name];args={key:value.get('value') for key,value in f.items() if key!='script'}
        if any(v is None for v in args.values()):self.gap('block-valued inline parameter '+name,n);return
        try:expanded=render(path.read_text(encoding='utf-8-sig'),args)
        except ValueError as e:self.emit('macro-argument',str(e),n);return
        if self.file not in self.reader_variables:
            source=Path(self.file)
            self.reader_variables[self.file]='\n'.join(re.findall(r'^@[A-Za-z_]\w*\s*=[^\r\n]*',source.read_text(encoding='utf-8-sig'),re.M)) if source.is_file() else ''
        locals_text=self.reader_variables[self.file]
        local_names=set(re.findall(r'^@([A-Za-z_]\w*)\s*=',locals_text,re.M))
        globals_text='\n'.join(line for line in self.globals_text.splitlines() if not ((match:=re.match(r'^@([A-Za-z_]\w*)\s*=',line)) and match[1] in local_names))
        parsed=parse_text(expanded,str(path),globals_text+'\n'+locals_text)
        parsed['ast']=[{'children':parsed['ast']}] if parsed['ast'] else []
        for d in parsed['diagnostics']:self.emit(d['key'],d['message'],n,d['severity'])
        if not parsed['ast']:return
        def relocate(items):
            for item in items:
                item['line']=max(1,item.get('line',1));relocate(children(item))
        relocate(children(parsed['ast'][0]))
        self.active.append('inline:'+name);saved=self.file;self.file=str(path)
        self.expansion_trace.append({'caller':saved,'line':n.get('line',0),'definition':str(path),'name':'inline:'+name,'scope':ctx.current,'root':ctx.root,'froms':list(ctx.froms),'prevs':list(ctx.prevs)})
        try:
            if handler:handler(parsed['ast'][0])
            elif kind=='definition':self.definition(category,parsed['ast'][0])
            else:self.script(children(parsed['ast'][0]),kind,ctx)
        finally:self.file=saved;self.active.pop()
    def expanded_structure(self,node,ctx):
        """Materialize nested declaration templates once for index AND validation.

        Source locations stay attached to template fields. Recursion is checked
        while the inline invocation is still active, including nested resources.
        """
        result={**node};result.setdefault('_source_path',self.file)
        if 'children' not in node:return result
        body=[]
        for child in children(node):
            if child.get('key')=='inline_script':
                def append(expanded):
                    for entry in children(expanded):
                        body.extend(children(self.expanded_structure({'children':[entry]},ctx)))
                self.expand_inline(child,'declaration-materialize',ctx,handler=append)
            else:body.append(self.expanded_structure(child,ctx))
        result['children']=body
        return result
    def parameters(self,k,n,kind,ctx,rule):
        if any(c.get('key')=='inline_script' for c in children(n)):
            return self.parameters(k,self.expanded_structure(n,ctx),kind,ctx,rule)
        if self.extended_parameter(k,n,kind,ctx):return
        if self.handoff_parameter(k,n,kind,ctx):return
        if self.creation_contract(k,n,ctx):return
        f=fields(n)
        if k=='enable_special_project':
            name=f.get('name',{}).get('value');location=f.get('location',{}).get('value')
            if name and location:
                scope=self.resolve(location,ctx,f['location']).current
                if scope!=UNKNOWN:self.project_locations[name]='|'.join(sorted(set(self.project_locations.get(name,scope).split('|'))|set(scope.split('|'))))
        if k=='export_resource_stockpile_to_variable':
            name=f.get('variable',{}).get('value')
            if name:
                for scope in ctx.current.split('|'):self.variable_types[(scope,name)]='number'
        if k in {'set_variable','change_variable','multiply_variable','divide_variable','subtract_variable'}:
            name=f.get('which',{}).get('value');value=f.get('value',{}).get('value')
            source=(value or '').removeprefix('var:');source_ctx=ctx
            if scoped_numeric_variable(source):
                prefix,_,source=source.rpartition('.');source_ctx=self.resolve(prefix,ctx,f.get('value',n))
            typed=all(self.variable_types.get((scope,source))=='number' for scope in source_ctx.current.split('|'))
            if name and (number(value) is not None or (value or '').startswith('trigger:') or typed):
                for scope in ctx.current.split('|'):self.variable_types[(scope,name)]='number'
        contracts={
          'set_variable':({'which','value'},set()),'change_variable':({'which','value'},set()),'multiply_variable':({'which','value'},set()),'divide_variable':({'which','value'},set()),'subtract_variable':({'which','value'},set()),'check_variable':({'which','value'},set()),
          'end_fleet_contract':(set(),{'initiator','reason'}),
          'set_subject_of':({'who'},{'use_demanded_terms','allow_instant_negotiation','preset'}),
          'observer_event':({'id'},{'days','random','scopes','random_days'}),
          'spawn_planet':({'class'},{'location','orbit_location','orbit_angle_offset','orbit_distance_offset','init_effect','size','name','orbit_angle','orbit_distance','has_ring','tile_blockers','init_effects','generate_random_name','spawn_beyond_gravity_well','flags','plane_offset','deposit_blockers','modifiers','check_overlap'}),
          'create_ambient_object':({'type'},{'location','scale','use_3d_location','entity_offset','entity_offset_angle','entity_offset_height','entity_face_object','entity_scale_to_size','play_animation_once','target','duration','is_wreck','base_angle_towards','effect','scripted_scale','appear_state'}),
          'add_modifier':({'modifier'},{'days','months','years','mult','multiplier','time_multiplier','clear_on_owner_change'}),
          'has_resource':({'type','amount'},set()),'resource_stockpile_compare':({'resource','value'},{'mult'}),
          'is_surveyed':({'who','status'},set()),'set_surveyed':({'surveyed'},{'surveyor'}),
          'create_fleet':(set(),{'name','effect','settings','parent','set_take_point'}),'create_ship':(set(),{'name','design','prefix','suffix','random_existing_design','random_existing_design_options','graphical_culture','graphical_culture_fallback','ftl','effect','upgradable','colonizer_species','age','rarity','growth_stage','create_colony'}),
          'create_megastructure':({'type'},{'location','owner','planet','build_percentage','graphical_culture','effect'}),
          'add_resource':(set(),{'mult','multiplier','cost','income','amount','resource'}|self.entities['strategic_resources']),
        }
        if k in EVENT_SCOPES or k=='observer_event':
            required={'id'};optional={'days','random','scopes','random_days','hours','months','years'}
            if f.get('id',{}).get('value'):
                event=f['id']['value']
                if event not in self.events:self.emit('missing-event','unknown event '+event,f['id'])
                elif k!='observer_event' and not set(EVENT_SCOPES[k].split('|')) & set(self.events[event].split('|')):self.emit('event-type','event type mismatch: '+event,f['id'])
                else:self.remember_event_call(event,n,ctx)
        elif k in contracts:required,optional=contracts[k]
        else:
            documented=set(rule.get('parameters',[]))
            if not documented:self.gap('parameter grammar not fully specified: '+k,n)
            for child in children(n):
                key=child.get('key','')
                if self.extended_child(k,child,ctx):continue
                if key not in documented:self.gap('undocumented parameter: '+k+'/'+key,child)
                elif key=='limit':self.script(children(child),'triggers',ctx)
                elif key in {'effect','init_effect'}:
                    output={'modify_species':'species','create_country':'country','create_army':'army','create_starbase':'starbase','create_ambient_object':'ambient_object','create_pop_group':'pop_group','spawn_planet':'planet','create_species':'species','create_leader':'leader','create_fleet':'fleet','create_ship':'ship'}.get(k,UNKNOWN)
                    self.script(children(child),'effects',ctx.push(output))
                elif key in {'amount','value','days','months','years','progress','mult','multiplier','min','max','duration','orbit_angle','orbit_distance','scale'} and 'value' in child:self.numeric(child,ctx)
                elif key in {'trigger','potential'} and 'children' in child:self.script(children(child),'triggers',ctx)
                elif 'children' in child:self.gap('parameter subgrammar not specified: '+k+'/'+key,child)
                elif key in {'who','owner','country','surveyor'} and child.get('value') and k not in {'create_species','create_country'}:self.compatible(self.resolve(child['value'],ctx,child).current,['pop_group','species'] if k=='habitability' and key=='who' else ['country'],child,k+' '+key)
                elif key in {'tech','technology'} and child.get('value'):self.ref('technology',child['value'],child)
                elif key=='type' and k in {'spawn_megastructure','create_megastructure'}:self.ref('megastructures',child.get('value'),child)
                elif key=='resource' and child.get('value'):self.ref('strategic_resources',child['value'],child)
            self.checked['engine_parameter_contracts']+=1
            return
        for key in required-f.keys():self.emit('missing-field',k+' requires '+key,n)
        for child in children(n):
            key=child.get('key')
            if key not in required|optional:self.emit('unknown-field',k+': unknown parameter '+str(key),child);continue
            if key in {'effect','init_effect','init_effects'}:
                out={'create_fleet':'fleet','create_ship':'ship','create_megastructure':'megastructure','spawn_planet':'planet','create_ambient_object':'ambient_object'}.get(k,ctx.current)
                self.script(children(child),'effects',ctx.push(out))
            elif key=='modifier':self.ref('static_modifiers',child.get('value'),child)
            elif k in EVENT_SCOPES and key=='scopes':
                if self.require_block(child):
                    for override in children(child):
                        label=override.get('key','').lower()
                        if label!='root' and not re.fullmatch(r'(from)+',label):self.emit('unknown-field','unknown event scope override '+label,override)
                        else:
                            value=self.scalar(override)
                            if value:self.resolve(value,ctx,override)
            elif key in {'resource','type'} and k in {'has_resource','resource_stockpile_compare'}:self.ref('strategic_resources',child.get('value'),child)
            elif k=='set_subject_of' and key=='who':
                value=self.scalar(child)
                if value is not None and value!='none':self.compatible(self.resolve(value,ctx,child).current,['country'],child,k+' '+key)
            elif k=='set_subject_of' and key=='preset':self.ref('agreement_presets',self.scalar(child),child)
            elif k=='set_subject_of' and key in {'use_demanded_terms','allow_instant_negotiation'}:
                if self.scalar(child) not in {'yes','no'}:self.emit('value',key+' expects yes/no',child)
            elif key in {'who','owner','surveyor'} and child.get('value'):self.compatible(self.resolve(child['value'],ctx,child).current,['country'],child,k+' '+key)
            elif key in {'status','surveyed'} and child.get('value') not in {'yes','no'}:self.emit('value',key+' expects yes/no',child)
            elif key in {'upgradable','create_colony'} and child.get('value') not in {'yes','no'}:self.emit('value',key+' expects yes/no',child)
            elif key=='rarity' and child.get('value') not in {'common','rare','epic','exceptional'}:self.emit('value','invalid space fauna rarity',child)
            elif key in {'age','growth_stage'}:self.numeric(child)
            elif key=='type' and k=='create_megastructure':self.ref('megastructures',child.get('value'),child)
            elif key=='class' and k=='spawn_planet':self.ref('planet_classes',child.get('value'),child)
            elif key=='parent' and k=='create_fleet':
                value=self.scalar(child)
                if value is not None and value!='none':self.compatible(self.resolve(value,ctx,child).current,['fleet'],child,'create_fleet parent')
            elif key=='appear_state' and k=='create_ambient_object':self.scalar(child)
            elif key=='scripted_scale' and k=='create_ambient_object':self.numeric(child,ctx)
            elif key in {'entity_offset','entity_offset_angle','entity_offset_height'}:self.numeric_range(child)
            elif key in {'size','duration','scale','orbit_angle_offset','orbit_distance_offset'}:self.numeric(child)
            elif key in {'generate_random_name','spawn_beyond_gravity_well','has_ring','use_3d_location','play_animation_once','is_wreck','check_overlap','set_take_point'}:
                if self.scalar(child) not in {'yes','no'}:self.emit('value',key+' expects yes/no',child)
            elif key in {'orbit_location','plane_offset','deposit_blockers','modifiers','flags'}:self.gap('creation parameter subgrammar: '+k+'/'+key,child)
            elif key in {'days','months','years','value','mult','multiplier','progress','amount'} and 'value' in child:self.numeric(child,ctx)
    def modifier(self,n,ctx,triggered=False):
        if not self.require_block(n):return
        for child in children(n):
            k=child.get('key','')
            if triggered and k in {'potential','trigger'}:
                if self.require_block(child):self.script(children(child),'triggers',ctx)
            elif triggered and k=='modifier':self.modifier(child,ctx)
            elif k=='inline_script':self.expand_inline(child,'modifier',ctx,handler=lambda body:self.modifier(body,ctx,triggered))
            elif k in {'icon','description','custom_tooltip'}:
                if self.scalar(child) is not None and k!='icon':self.localisation(child)
            elif k=='icon_frame':self.numeric(child)
            elif k in {'show_only_custom_tooltip','hide_from_country_list','important'}:
                if self.scalar(child) not in {'yes','no'}:self.emit('value',k+' expects yes/no',child)
            elif k in {'mult','multiplier'}:self.numeric(child)
            elif k not in self.modifiers:self.emit('unknown-modifier','unknown modifier '+k,child)
            else:self.numeric(child)
    def resources(self,n,ctx):
        if not self.require_block(n):return
        for child in children(n):
            k=child.get('key')
            if k=='inline_script':self.expand_inline(child,'resources',ctx,handler=lambda body:self.resources(body,ctx))
            elif k=='category':self.ref('economic_categories',child.get('value'),child)
            elif k in {'cost','upkeep','produces','logistics'}:
                if not self.require_block(child):continue
                for value in children(child):
                    if value.get('key')=='trigger':
                        if self.require_block(value):self.script(children(value),'triggers',ctx)
                    elif value.get('key') in {'multiplier','mult'}:self.numeric(value)
                    else:self.ref('strategic_resources',value.get('key'),value);self.numeric(value,ctx)
            else:self.emit('unknown-field','resources: unknown field '+str(k),child)
    def observed_field(self,category,child,ctx):
        if self.declaration_subgrammar(category,child,ctx):return True
        k=child.get('key','');schema=self.declarations.get(category,{}).get(k)
        extra_weapon_fields={'military_power_multiplier','missile_armor','missile_evasion','missile_health','missile_retarget_range','missile_shield','missile_speed','total_fire_time','windup'}
        if category=='component_templates' and k in extra_weapon_fields:self.numeric_range(child);return True
        if not schema:return False
        form='block' if 'children' in child else 'scalar'
        if form not in schema['forms']:
            self.gap('declaration form not seen in native examples: '+category+'/'+k,child);return True
        if form=='scalar':
            types=schema['scalar_types']
            if types==['boolean'] and child.get('value') not in {'yes','no'}:self.emit('value',k+' expects yes/no',child)
            elif types==['number']:self.numeric(child)
            refs={'required_component_set':'component_sets','component_set':'component_sets','ship_size':'ship_sizes','upgrades_to':'ship_sizes' if category=='ship_sizes' else 'component_templates','category':'pop_categories' if category=='pop_jobs' else None,'possible_precalc':'game_rules'}
            if refs.get(k):self.ref(refs[k],child.get('value'),child)
        elif all('key' not in i and 'children' not in i for i in children(child)):
            pass  # free tag/name lists have no script semantics
        else:self.gap('declaration subgrammar requires review: '+category+'/'+k,child)
        self.checked['native_declaration_fields']+=1
        return True
    def reference_list(self,n,category,logical=False):
        if not self.require_block(n):return
        for entry in children(n):
            key=entry.get('key')
            if logical and key in {'OR','AND','or','and'}:self.reference_list(entry,category,True)
            elif key=='inline_script':self.expand_inline(entry,'reference-list',Context('none','none'),handler=lambda body:self.reference_list(body,category,logical))
            elif 'key' in entry or 'children' in entry:self.emit('structure',n.get('key','reference list')+' expects scalar references or documented AND/OR groups',entry)
            else:self.ref(category,self.scalar(entry),entry)
    def definition(self,category,n):
        if category in {'scripted_triggers','scripted_effects','scripted_variables'}:return
        if category=='events':self.event(n);return
        if category=='name_lists':self.name_list(n);return
        if category=='component_slot_templates':self.slot_definition(n);return
        if category=='global_ship_designs':return  # validated with cross-file contracts after traversal
        if self.added_declaration(category,n):return
        roots={'buildings':'planet','pop_jobs':'pop_group','decisions':'planet','megastructures':'country','technology':'country','edicts':'country','ascension_perks':'country','traditions':'country','ship_sizes':'country','component_templates':'ship','component_sets':'none','section_templates':'country','economic_categories':'country','special_projects':'country','on_actions':UNKNOWN,'static_modifiers':'all'}
        if category not in roots:
            self.gap('definition grammar not implemented: common/'+category,n);return
        if category=='static_modifiers':self.modifier(n,Context(UNKNOWN,UNKNOWN));return
        scope=roots[category];ctx=Context(scope,scope)
        if category=='on_actions':ctx=self.engine_contract(n.get('key',''),'on_actions',n) or ctx
        if category=='megastructures':ctx=Context('country','country',('fleet',))
        if category=='special_projects':
            event_type=fields(n).get('event_scope',{}).get('value','country_event');scope=EVENT_SCOPES.get(event_type,UNKNOWN);ctx=Context(scope,scope)
        if category=='component_templates':ctx=Context('ship','ship',('country',))
        if category=='component_templates':self.weapon_contract(n)
        # Top-level fields verified against vanilla READMEs and native definitions.
        mega_scalar=set('entity construction_entity portrait place_entity_on_planet_plane build_type build_time construction_blocks_and_blocked_by use_planet_resource construction_scale show_galactic_map_icon show_in_outliner icon order_icon hide_name upgrade_desc can_be_dismantled can_be_dismantled_by_non_owner can_prevent_crisis_terraformation dismantle_time victory_score custom_tooltip custom_tooltip_with_modifiers overclock_cooldown overclock_loc_key'.split())
        building_scalar=set('base_buildtime category position potential_only destroy_trigger owner_condition show_tech_unlock_if build_sound icon base_capital is_capital unique capital can_build disabled use_building_from_destroyed_colony always_display should_show_produces can_demolish can_be_ruined can_be_disabled position_priority capital_tier'.split())
        for child in children(n):
            k=child.get('key','')
            if k.startswith('@'):continue
            if category=='special_projects' and k in {'on_success','on_start','on_progress_25','on_progress_50','on_progress_75','on_fail','on_cancel','abort_trigger','fail_trigger'}:
                location=self.project_locations.get(fields(n).get('key',{}).get('value',n.get('key')),'?')
                success=k in {'on_success','on_start','on_progress_25','on_progress_50','on_progress_75'}
                # documentation.txt defines THIS/FROM/PREV, but does not
                # specify ROOT. Do not export an invented ROOT into events.
                callback=Context(scope,UNKNOWN,(location,),('country',)) if success else Context('country',UNKNOWN,(scope,location))
                self.script(children(child),'triggers' if k.endswith('trigger') else 'effects',callback);continue
            if k=='inline_script':self.expand_inline(child,'definition',ctx,category);continue
            if category=='section_templates' and k=='component_slot':
                for missing in {'name','template'}-fields(child).keys():self.emit('missing-field','component_slot requires '+missing,child)
                for slot in children(child):
                    if slot.get('key') not in {'name','template','locatorname','is_side_slot','rotation','section_instance','position'}:self.emit('unknown-field','unknown component_slot field '+slot.get('key',''),slot)
                    elif slot.get('key')=='template':self.ref('component_slot_templates',slot.get('value'),slot)
                    elif slot.get('key') in {'rotation','section_instance'}:self.numeric(slot)
                    elif slot.get('key')=='position':
                        for axis in children(slot):
                            if axis.get('key') not in {'x','y','z'}:self.emit('unknown-field','unknown position axis '+axis.get('key',''),axis)
                            else:self.numeric(axis)
                    elif slot.get('key')=='is_side_slot' and slot.get('value') not in {'yes','no'}:self.emit('value','is_side_slot expects yes/no',slot)
                continue
            if category=='buildings' and k in {'building_sets','convert_to'}:
                if k=='convert_to':
                    for v in values(child):self.ref('buildings',v,child)
                elif any(not re.fullmatch(r'\w+',v) for v in values(child)):self.emit('value','building_sets expects tag names',child)
                continue
            if k in {'prerequisites','upgrade_from','upgrades'}:
                target='technology' if k=='prerequisites' else category
                self.reference_list(child,target,k=='prerequisites')
            elif k=='resources':self.resources(child,Context('ship|ship_growth_stage','ship|ship_growth_stage',('country',)) if category=='ship_sizes' else ctx)
            elif k in {'ai_weight','ai_weights','weight_modifier','ai_allow','ai_chance','weight'}:
                wc=Context('country','country') if category=='component_templates' else ctx
                self.weighted(child,'triggers',wc)
            elif k in {'country_modifier','planet_modifier','system_modifier','modifier','pop_modifier','ship_modifier','triggered_ship_modifier','triggered_ship_design_modifier','triggered_country_modifier','triggered_planet_modifier','triggered_pop_modifier','triggered_modifier'}:
                mc=Context('megastructure','megastructure') if category=='megastructures' and k.startswith('triggered_') else ctx
                if category=='component_templates' and k=='triggered_ship_design_modifier':mc=Context('design','design',('country',))
                self.modifier(child,mc,k.startswith('triggered_'))
            elif category=='component_templates' and k in {'potential','possible'}:
                if self.require_block(child):self.script(children(child),'triggers',Context('design','design',('country',)))
            elif category=='megastructures' and k in {'possible','context_menu_potential','potential','dismantle_potential','dismantle_possible','should_ai_dismantle'}:
                s='country' if k=='potential' else ('megastructure' if 'dismantle' in k else 'galactic_object')
                self.script(children(child),'triggers',Context(s,s,('fleet',) if k=='potential' else ('country','fleet')))
            elif category=='megastructures' and k=='placement_rules':
                for sub in children(child):
                    if sub.get('key')=='planet_possible':self.script(children(sub),'triggers',Context('planet','galactic_object',('country','fleet'),('galactic_object',)))
                    else:self.gap('placement_rules field '+sub.get('key',''),sub)
            elif category=='megastructures' and k.startswith(('on_build_','on_dismantle_')):self.script(children(child),'effects',Context('galactic_object','galactic_object',('country','megastructure','fleet')))
            elif category=='ship_sizes' and k in {'potential_country','potential_construction','possible_construction'}:
                s='country' if k=='potential_country' else 'starbase|planet|ship|megastructure'
                self.script(children(child),'triggers',Context(s,s,('country',)))
            elif category=='pop_jobs' and k=='possible_pre_triggers':
                for pre in children(child):
                    if pre.get('key') not in {'has_owner','is_enslaved','is_being_purged','is_being_assimilated','has_planet','is_sapient','is_robotic'}:self.emit('unknown-field','unknown job pre-trigger '+pre.get('key',''),pre)
                    elif pre.get('value') not in {'yes','no'}:self.emit('value','job pre-trigger expects yes/no',pre)
            elif k=='triggered_desc':self.description(child,ctx)
            elif k in {'potential','allow','possible','trigger','destroy_trigger','active','visible','is_valid','can_use','ai_allow','abort_trigger','fail_trigger','valid_for_country'} and 'children' in child:self.script(children(child),'triggers',Context('country','country') if k=='valid_for_country' or (category=='special_projects' and k=='abort_trigger') else ctx)
            elif k in {'effect','on_queued','on_unqueued','on_built','on_destroy','on_destroyed','on_enabled','on_disabled','on_build_complete','on_research_complete','on_success','on_fail','on_cancel','on_start','on_progress_25','on_progress_50','on_progress_75'}:self.script(children(child),'effects',ctx)
            elif category=='on_actions' and k in {'events','random_events'}:
                for v in values(child):
                    if v not in self.events:self.emit('missing-event','unknown event '+v,child)
            elif category=='on_actions' and k=='effect':self.script(children(child),'effects',ctx)
            elif (category=='megastructures' and k in mega_scalar) or (category=='buildings' and k in building_scalar):
                if k.startswith('custom_tooltip'):self.localisation(child)
                elif k in {'build_time','base_buildtime','position','construction_scale','dismantle_time','victory_score','overclock_cooldown'}:self.numeric(child)
                elif k in {'place_entity_on_planet_plane','use_planet_resource','hide_name','show_galactic_map_icon','show_in_outliner','can_be_dismantled','can_be_dismantled_by_non_owner','can_prevent_crisis_terraformation','unique','is_capital','capital','always_display'} and child.get('value') not in {'yes','no'}:self.emit('value',k+' expects yes/no',child)
            elif category=='megastructures' and k=='entity_offset':
                for axis in children(child):
                    if axis.get('key') not in {'x','y','z'}:self.emit('unknown-field','unknown coordinate '+axis.get('key',''),axis)
                    else:self.numeric(axis)
            elif category=='megastructures' and k in {'starbase','dismantle_cost','overclock_types','tooltip_best_systems'}:self.gap('definition subgrammar: '+category+'/'+k,child)
            elif self.observed_field(category,child,ctx):pass
            elif category in {'megastructures','buildings'}:self.emit('unknown-field',category+': unknown field '+k,child)
            else:self.gap('definition field: '+category+'/'+k,child)
    def event(self,n):
        k=n.get('key','')
        if k in {'namespace','@'} or k.startswith('@'):return
        if k not in EVENT_SCOPES:self.emit('unknown-event-type','unknown event type '+k,n);return
        scope=EVENT_SCOPES[k]
        # A scope-free event has no initial THIS object. The installed export
        # does not define how ROOT is initialized when entering its first
        # object iteration; do not invent a permanent, known-invalid none ROOT.
        name=fields(n).get('id',{}).get('value');saved=self.targets
        for variant in self.event_variants(name,scope,fields(n).get('is_triggered_only',{}).get('value')=='yes'):
            self.targets=dict(variant['targets'])
            self.event_body({'children':self.inherited_event_fields(n)},variant['context'])
        self.targets=saved
    def inherited_event_fields(self,n,seen=()):
        local=children(n);base=fields(n).get('base');result=[]
        if base:
            name=self.scalar(base)
            if name in seen:self.emit('inheritance-cycle','event inheritance cycle: '+str(name),base)
            elif name not in self.event_defs:self.emit('missing-event','unknown base event '+str(name),base)
            else:
                parent,path=self.event_defs[name]
                saved=self.file;self.file=path
                result=copy.deepcopy(self.inherited_event_fields(parent,seen+(name,)))
                def locate(nodes):
                    for item in nodes:item.setdefault('_source_path',path);locate(children(item))
                locate(result)
                self.file=saved
        clears={x['key'][:-6] for x in local if x.get('key','').endswith('_clear') and x.get('value')=='yes'}
        replacements={x.get('key') for x in local if 'value' in x}
        result=[x for x in result if x.get('key') not in clears|replacements|{'base'}]
        return result+[x for x in local if x.get('key')!='base']
    def conditional_picture(self,n,ctx):
        if not self.require_block(n):return
        for item in children(n):
            key=item.get('key')
            if key=='trigger':
                if self.require_block(item):self.script(children(item),'triggers',ctx)
            elif key=='picture':
                if 'children' in item:self.conditional_picture(item,ctx)
                else:self.scalar(item)
            elif key=='inline_script':self.expand_inline(item,'conditional-picture',ctx,handler=lambda body:self.conditional_picture(body,ctx))
            else:self.emit('unknown-field','unknown conditional picture field '+str(key),item)
    def option_icon(self,n,ctx):
        if 'children' not in n:self.scalar(n);return
        for item in children(n):
            key=item.get('key')
            if key in {'icon','icon_background'}:self.scalar(item)
            elif key=='text':self.localisation(item)
            elif key=='trigger':
                if self.require_block(item):self.script(children(item),'triggers',ctx)
            elif key=='inline_script':self.expand_inline(item,'option-icon',ctx,handler=lambda body:self.option_icon(body,ctx))
            else:self.emit('unknown-field','unknown option icon field '+str(key),item)
    def event_body(self,n,ctx):
        scalar=set('id title desc picture location show_sound hide_window is_triggered_only fire_only_once major major_triggered_only diplomatic notification auto_opens duration show_on_map_trigger immediate_only keep_deposit_effects_in_description diplomatic_title event_window_type situation event_chain archaeology force_open specimen auto_select trackable is_advisor_event is_test_event event_picture_background notification_event_icon notification_event_icon_frame event_message_type message_desc difficulty astral_rift first_contact espionage_operation'.split())
        booleans={'hide_window','is_triggered_only','fire_only_once','major','major_triggered_only','diplomatic','notification','auto_opens','trackable','is_advisor_event','is_test_event','astral_rift','first_contact','espionage_operation'}
        clearable={'desc','option','trigger','picture','show_sound','title','immediate','after'}
        for child in children(n):
            name=child.get('key','')
            if name=='inline_script':self.expand_inline(child,'event-body',ctx,handler=lambda body:self.event_body(body,ctx))
            elif name=='trigger':self.script(children(child),'triggers',ctx)
            elif name=='picture' and 'children' in child:self.conditional_picture(child,ctx)
            elif name=='triggered_desc':self.description(child,ctx)
            elif name.endswith('_clear') and name[:-6] in clearable:
                if self.scalar(child) not in {'yes','no'}:self.emit('value',name+' expects yes/no',child)
            elif name=='abort_trigger':self.script(children(child),'triggers',ctx)
            elif name in {'immediate','after'}:
                if not self.require_block(child):continue
                statements=[]
                for effect in children(child):
                    if effect.get('key')=='run_in_ai_mode':
                        if self.scalar(effect) not in {'yes','no'}:self.emit('value','run_in_ai_mode expects yes/no',effect)
                    else:statements.append(effect)
                self.script(statements,'effects',ctx)
            elif name=='option':
                for opt in children(child):
                    key=opt.get('key')
                    if key in {'name','custom_tooltip'}:self.localisation(opt)
                    elif key in {'trigger','allow','highlight','exclusive_trigger'}:self.script(children(opt),'triggers',ctx)
                    elif key=='ai_chance':self.weighted(opt,'triggers',ctx)
                    elif key in {'default_hide_option','is_dialog_only','hide_option_if_not_allowed','default'}:self.boolean(opt)
                    elif key=='icon':self.option_icon(opt,ctx)
                    elif key in {'response_text','sound','show_sound','tag'}:self.scalar(opt)
                    else:self.script([opt],'effects',ctx)
            elif name in scalar:
                if name in {'title','desc','message_desc'}:self.description(child,ctx)
                elif name in booleans:
                    if self.scalar(child) not in {'yes','no'}:self.emit('value',name+' expects yes/no',child)
                elif name in {'difficulty','duration','notification_event_icon_frame'}:self.numeric(child)
                elif 'children' in child:self.gap('event metadata subgrammar: '+name,child)
            elif name in {'mean_time_to_happen','weight_multiplier'}:self.weighted(child,'triggers',ctx)
            elif name=='picture_event_data':self.picture_event_data(child,ctx)
            elif name in {'sound','pre_triggers','custom_gui','custom_gui_option'}:self.gap('event subgrammar: '+name,child)
            else:self.emit('unknown-field','event: unknown field '+name,child)
    def description(self,n,ctx):
        if 'value' in n:self.localisation(n);return
        for child in children(n):
            key=child.get('key','')
            if key=='text':self.localisation(child)
            elif key in {'trigger','exclusive_trigger'}:
                for condition in children(child):
                    if condition.get('key')=='switch':
                        selector=fields(condition).get('trigger',{});name=selector.get('value','')
                        if name not in self.rules['triggers']:self.emit('unknown-trigger','unknown description switch selector '+name,selector)
                        else:self.compatible(ctx.current,self.rules['triggers'][name]['input'],selector,name)
                        for branch in children(condition):
                            if branch.get('key')!='trigger':self.description(branch,ctx)
                    else:self.script([condition],'triggers',ctx)
            elif key=='switch':self.description({'children':[{'key':'trigger','children':[child]}]},ctx)
            elif key=='show_sound':self.scalar(child)
            elif key in {'if','else_if','else'}:
                if not self.require_block(child):continue
                if key!='else' and 'limit' not in fields(child):self.emit('missing-field',key+' description requires limit',child)
                for branch in children(child):
                    if branch.get('key')=='limit':
                        if self.require_block(branch):self.script(children(branch),'triggers',ctx)
                    else:self.description({'children':[branch]},ctx)
            elif key in {'desc','random_valid','first_valid'}:self.description(child,ctx)
            else:self.emit('unknown-field','unknown description field '+key,child)

from reporting import traced
for _method,_kind in [('definition','definition'),('script','script'),('expand','scripted-call'),('event_body','event-body')]:
    setattr(Checker,_method,traced(getattr(Checker,_method),_kind))

def collect_files(root):
    result={}
    for folder in ['common','events','interface','gfx']:
        base=root/folder
        if not base.exists():continue
        for p in sorted(base.rglob('*')):
            allowed={'.txt'} if folder in {'common','events'} else {'.txt','.gfx','.asset','.gui'}
            # These files contain prose, license text, credits or UI text substitutions.
            # Runtime portrait, projectile and root common/*.txt files remain in the corpus.
            prose=p.name.lower() in {'how_to_make_new_ships.txt','license.txt','reference.txt','startup_info.txt'} or (folder=='interface' and p.name.lower().startswith('credits'))
            if p.is_file() and p.suffix.lower() in allowed and not prose and not p.name.lower().startswith(('99_readme','readme')):result[p.relative_to(root).as_posix().lower()]=p
    return result

def load(checker,game,mods,cache=None):
    final=collect_files(game);layers=[game]+mods
    checker.asset_roots=layers
    checker.asset_replace_paths={}
    for root in layers:
        colors=root/'flags/colors.txt'
        if colors.exists():
            checker.input_manifest.append({'relative_path':'flags/colors.txt','path':str(colors),'sha256':hashlib.sha256(colors.read_bytes()).hexdigest()})
            for node in parse_text(colors.read_text(encoding='utf-8-sig'))['ast']:
                checker.entities['flag_colors'].update(x.get('key','') for x in children(node))
    for mod in mods:
        descriptor=mod/'descriptor.mod'
        if descriptor.exists():
            checker.input_manifest.append({'relative_path':'descriptor.mod','path':str(descriptor),'sha256':hashlib.sha256(descriptor.read_bytes()).hexdigest(),'descriptor':True})
            parsed=parse_text(descriptor.read_text(encoding='utf-8-sig'),str(descriptor))
            for n in parsed['ast']:
                if n.get('key')=='replace_path':
                    prefix=n.get('value','').strip('/').lower()+'/'
                    checker.asset_replace_paths.setdefault(str(mod),[]).append(prefix)
                    final={k:v for k,v in final.items() if not k.startswith(prefix)}
        final.update(collect_files(mod))
    final=dict(sorted(final.items()))
    checker.final_files=final
    checker.prepare_engine_entries(game)
    # CSV statistics participate in the same-path overlay, independently of TXT.
    import csv,io
    csvs={}
    for root in layers:
        for p in sorted((root/'common/component_templates').glob('*.csv')):csvs[p.name.lower()]=p
    for p in csvs.values():
        checker.input_manifest.append({'relative_path':'common/component_templates/'+p.name,'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'weapon_csv':True})
        rows=list(csv.reader(io.StringIO(p.read_text(encoding='utf-8-sig')),delimiter=';'))
        header=next((r for r in rows if r and r[0]=='key'),None)
        if header:
            for r in rows:
                if r and r[0] not in {'key',''} and not r[0].startswith('#'):checker.weapon_stats[r[0]]=dict(zip(header,r))
    globals_text='\n'.join(p.read_text(encoding='utf-8-sig') for rel,p in sorted(final.items()) if rel.startswith('common/scripted_variables/'))
    checker.globals_text=globals_text
    checker.inline={rel[len('common/inline_scripts/'):-4]:p for rel,p in final.items() if rel.startswith('common/inline_scripts/')}
    templates={rel:p for rel,p in final.items() if rel.startswith('common/inline_scripts/')}
    checker.template_files=len(templates)
    checker.input_manifest.extend({'relative_path':rel,'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'template':True} for rel,p in templates.items())
    final={rel:p for rel,p in final.items() if rel not in templates}
    checker.global_variables_hash=hashlib.sha256(globals_text.encode()).hexdigest()
    for root in layers:
        for p in (root/'localisation').rglob('*.yml'):
            checker.input_manifest.append({'relative_path':p.relative_to(root).as_posix(),'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'localisation':True})
            text=p.read_text(encoding='utf-8-sig',errors='replace');checker.loc.update(re.findall(r'^\s*([^#\s:]+):(?:\d+)?\s*"',text,re.M))
    checker.selected_localisation(layers)
    target_paths=set(collect_files(mods[-1]).values()) if mods else set(final.values())
    checker.mod_files={str(p) for p in target_paths};result={};pending=[];cache_data={}
    if cache:
        cache.mkdir(parents=True,exist_ok=True)
        from native_parser import parser_path
        identity=hashlib.sha256((checker.global_variables_hash+hashlib.sha256(parser_path().read_bytes()).hexdigest()).encode()).hexdigest()[:20]
    for rel,p in final.items():
        digest=hashlib.sha256(p.read_bytes()).hexdigest();checker.input_manifest.append({'relative_path':rel,'path':str(p),'sha256':digest})
        cp=cache/(identity+'-'+hashlib.sha256(str(p).encode()).hexdigest()[:16]+'-'+digest+'.json') if cache else None
        if cp and cp.exists():result[str(p)]=explain_braces(json.loads(cp.read_text(encoding='utf8')),p.read_text(encoding='utf-8-sig'))
        else:pending.append(p);cache_data[str(p)]=cp
    if pending: print(f'Tiger Stellar: parsing {len(pending)} files',file=sys.stderr)
    for item in parse_many(pending,globals_text=globals_text):
        result[item['path']]=item;cp=cache_data[item['path']]
        if cp:
            temporary=cp.with_suffix('.'+uuid.uuid4().hex+'.tmp')
            try:
                temporary.write_text(json.dumps(item,ensure_ascii=False),encoding='utf8');temporary.replace(cp)
            finally:temporary.unlink(missing_ok=True)
    # Top-level event templates contribute event IDs and base definitions before
    # reference checks, including templates loaded from dependencies/vanilla.
    checker.vanilla_parse_diagnostics=[]
    for rel,p in final.items():
        if not rel.startswith(('events/','common/component_templates/')):continue
        original=result[str(p)]['ast'];expanded=[];checker.file=str(p);diagnostic_start=len(checker.diagnostics)
        def expand_root(node):
            if node.get('key')=='inline_script':
                checker.expand_inline(node,'event-root',Context('none','none'),handler=lambda body:[expand_root(child) for child in children(body)])
            else:
                def source(nodes):
                    for item in nodes:item.setdefault('_source_path',checker.file);source(children(item))
                source([node])
                expanded.append(node)
        for node in original:expand_root(node)
        result[str(p)]={**result[str(p)],'ast':expanded}
        if p not in target_paths:
            checker.vanilla_parse_diagnostics.extend(dict(d,source='event-template-index') for d in checker.diagnostics[diagnostic_start:])
            del checker.diagnostics[diagnostic_start:]
    target=[];checker.file_records=[];economic=[];initializers={};checker.syntax_files=len(result)
    for rel,p in final.items():
        item=result[str(p)];ast=item['ast'];category=rel.split('/')[1] if rel.startswith('common/') else rel.split('/')[0]
        if category=='component_templates':
            saved_file=checker.file;checker.file=str(p);diag_start=len(checker.diagnostics)
            ast=[checker.expanded_structure(n,Context('ship','ship',('country',))) for n in ast]
            if p not in target_paths:
                checker.vanilla_parse_diagnostics.extend(dict(d,source='component-template-materialize') for d in checker.diagnostics[diag_start:]);del checker.diagnostics[diag_start:]
            checker.file=saved_file
        if category in {'gfx','interface'}:checker.index_graphics(ast,str(p))
        if rel.startswith(('common/governments/','common/patrons/')):category='/'.join(rel.split('/')[1:-1])
        checker.file_records.append((str(p),category,ast))
        if category=='interface':
            def index_containers(nodes):
                for container in nodes:
                    if container.get('key','').lower() in {'containertype','windowtype','containerwindowtype'}:
                        name=fields(container).get('name',{}).get('value')
                        if name:checker.entities['gui_containers'].add(name)
                    index_containers(children(container))
            index_containers(ast)
        for n in ast:
            key=n.get('key');
            if not key or key.startswith('@'):continue
            if category=='events':
                if key in EVENT_SCOPES:
                    event_id=fields(n).get('id',{}).get('value')
                    if event_id:
                        checker.events[event_id]=EVENT_SCOPES[key];checker.event_defs[event_id]=(n,n.get('_source_path',str(p)))
                        checker.definitions['events'][event_id]={**n,'_source_path':str(p)}
            elif category in {'scripted_effects','scripted_triggers'}:
                checker.scripts['effects' if category=='scripted_effects' else 'triggers'][key]=(n,str(p));checker.entities[category].add(key)
            else:
                identifier=fields(n).get('key',{}).get('value',key) if category in {'component_sets','component_templates','section_templates','special_projects'} else key
                if category=='global_ship_designs':identifier=fields(n).get('name',{}).get('value',identifier)
                checker.entities[category].add(identifier)
                checker.definitions[category][identifier]={**n,'_source_path':str(p)}
                checker.definition_history[category,identifier].append({'path':str(p),'line':n.get('line',0)})
                if category=='global_ship_designs':checker.entities[category].add(fields(n).get('name',{}).get('value',''))
                if category=='economic_categories':economic.append(n)
                if category=='solar_system_initializers':initializers[key]=n
        if p in target_paths:
            target.append((str(p),category,ast))
            for d in item['diagnostics']:checker.emit(d['key'],d['message'],d,d['severity'],str(p))
        else:
            checker.vanilla_parse_diagnostics.extend(dict(d,path=str(p)) for d in item['diagnostics'])
    checker.index_initializer_roots(initializers)
    # Native economic_categories README specifies these engine-generated names.
    # Derive from the final overlaid definitions, including mod-only categories.
    for node in economic:
        for suffix in ['add','mult']:
            for kind in values(fields(node).get('generate_'+suffix+'_modifiers',{})):
                if kind not in {'produces','upkeep','cost'}:continue
                checker.modifiers.add(node['key']+'_'+kind+'_'+suffix)
                for resource in checker.entities['strategic_resources']:
                    checker.modifiers.add(node['key']+'_'+resource+'_'+kind+'_'+suffix)
    # Engine-generated job modifiers depend on mod entity definitions.
    for job in checker.entities['pop_jobs']:
        for suffix in ['add','per_pop','per_crime','per_pop_group','per_planet_size','per_district','per_pop_infrastructure','per_free_job']:checker.modifiers.add('job_'+job+'_'+suffix)
        for resource in checker.entities['strategic_resources']:
            for kind in ['produces','upkeep']:
                for suffix in ['add','mult']:checker.modifiers.add('job_'+job+'_'+resource+'_'+kind+'_'+suffix)
    return target

def analyze(checker,target,args):
    # Infer global target type constraints to a fixed point before emitting
    # semantic diagnostics. This does not prove runtime target availability.
    inference_iterations=0
    if not args.syntax_only:
        checker.inference_mode=True
        discovery=checker.discovery_roots(target)
        # Conservative call-graph filtering: a root without a direct global
        # writer, a writer script, any macro, or any inline expansion cannot
        # contribute type facts. Final semantic validation still visits all.
        writers=set()
        def can_write(node):
            keys={'save_event_target_as','save_global_event_target_as','enable_special_project','inline_script'}|set(EVENT_SCOPES)|writers|getattr(checker,'discovery_needed',set())
            if node.get('key')=='arrival_effect' and node.get('value') in keys:return True
            if 'macro_parts' in node:
                source=''.join('$'+p['text']+'$' if p['parameter'] else p['text'] for p in node['macro_parts'])
                tokens=set(re.findall(r'([\w.$]+)\s*=',source))
                return bool(tokens&keys) or any('$' in token for token in tokens)
            if node.get('key') in keys:return True
            return any(can_write(child) for child in children(node))
        definitions=[(name,node) for group in checker.scripts.values() for name,(node,_) in group.items()]
        while True:
            updated=writers|{name for name,node in definitions if can_write(node)}
            if updated==writers:break
            writers=updated
        inference_target=[(path,category,[node for node in ast if can_write(node)]) for path,category,ast in target]
        checker.inference_interesting=writers|getattr(checker,'discovery_needed',set())
        for iteration in range(16):
            old_events=checker.event_contexts;checker.next_event_contexts={};checker.script_contexts={}
            old_projects=checker.project_locations.copy();old=checker.global_targets.copy();old_unknown=checker.global_unknown_targets.copy();checker.global_unknown_targets.clear()
            for path,category,ast in inference_target+discovery:
                checker.file=path
                if category in {'gfx','interface'}:continue
                for n in ast:
                    checker.targets={};checker.definition(category,n)
            inference_iterations=iteration+1
            checker.event_contexts=checker.joined_event_contexts()
            if checker.global_targets==old and checker.global_unknown_targets==old_unknown and checker.project_locations==old_projects and checker.event_contexts==old_events:break
        checker.inference_mode=False;checker.checked.clear();checker.expansion_trace.clear();checker.invoked.clear()
        if inference_iterations==16 and (checker.global_targets!=old or checker.global_unknown_targets!=old_unknown or checker.event_contexts!=old_events or checker.project_locations!=old_projects):checker.gap('context/target inference did not converge within 16 iterations')
    for path,category,ast in target:
        checker.file=path
        if args.syntax_only:continue
        if category in {'gfx','interface'}:
            if Path(path).suffix.lower()=='.gui':checker.gap('GUI behavioral semantic grammar is not implemented')
            else:checker.graphics(ast)
            continue
        for n in ast:
            checker.targets={}
            checker.definition(category,n)
    checker.uninvoked_mod_scripts=[]
    if not args.syntax_only:
        checker.ship_crosschecks(target)
        for kind,definitions in checker.scripts.items():
            for name,(node,path) in definitions.items():
                if path in checker.mod_files and (kind,name) not in checker.invoked:
                    checker.file=path
                    entry=checker.engine_contract(name,kind,node)
                    if entry:
                        checker.script(children(node),kind,entry);continue
                    if checker.check_script_contexts(kind,name,node):continue
                    checker.uninvoked_mod_scripts.append({'kind':kind,'name':name,'path':path,'line':node.get('line',0)})
                    checker.file=path
                    contract=checker.infer_script_inputs(node,kind)
                    if contract:
                        checker.inferred_script_inputs[name]=contract
                        checker.script(children(node),kind,Context(contract,contract))
                    else:
                        checker.gap('scripted definition has no inferred caller context: '+name,node)
                        if 'macro_parts' not in node:checker.script(children(node),kind,Context(UNKNOWN,UNKNOWN))
    checker.input_manifest.extend(checker.asset_inputs.values())
    return inference_iterations


def run(args):
    start=time.monotonic();rules=json.loads(args.rules.read_text(encoding='utf8'));checker=Checker(rules)
    checker.language=getattr(args,'language',None)
    launcher=args.game/'launcher-settings.json'
    if not launcher.exists():raise ValueError('wrong-game: Tiger Stellar requires a Stellaris installation with launcher-settings.json')
    settings=json.loads(launcher.read_text(encoding='utf-8-sig'))
    if settings.get('gameId')!='stellaris':raise ValueError('wrong-game: Tiger Stellar accepts only Stellaris, got '+str(settings.get('gameId')))
    version=settings['rawVersion']
    if version!=rules['game_version']:checker.emit('version-mismatch',f'rules are {rules["game_version"]}, installed game is {version}; validation continues',severity='warning',path=str(args.game))
    target=load(checker,args.game,args.dependency+[args.mod] if args.mod else [],args.cache)
    input_state='script-targets'
    if args.mod and not target:
        nested=any((args.mod/'mod'/part).exists() for part in ['common','events'])
        input_state='parent-directory' if nested else ('metadata-assets-only' if (args.mod/'descriptor.mod').exists() else 'no-script-targets')
        checker.emit('empty-target','No script target files found under --mod ('+input_state+'); this is not semantic validation evidence',severity='warning',path=str(args.mod))
    inference_iterations=analyze(checker,target,args)
    from project_reports import constraints,overlay
    policy=[]
    if getattr(args,'project_constraints',None):
        config=args.project_constraints;checker.input_manifest.append({'relative_path':config.name,'path':str(config),'sha256':hashlib.sha256(config.read_bytes()).hexdigest(),'project_constraints':True})
        policy=constraints(checker,json.loads(config.read_text(encoding='utf-8-sig')))
    if getattr(args,'overrides_output',None):
        output=args.overrides_output;output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(overlay(checker,args.game,args.dependency+[args.mod] if args.mod else []),ensure_ascii=False,indent=2),encoding='utf8')
    # Assets and optional overlay evidence discovered after the semantic pass.
    have={x['path'] for x in checker.input_manifest}
    checker.input_manifest.extend(v for k,v in checker.asset_inputs.items() if k not in have)
    from reporting import binding,attribute,coverage_summary,diagnostic_delta
    controls=None
    if getattr(args,'native_control',False) and args.mod and not args.syntax_only:
        control=Checker(rules)
        control.language=getattr(args,'language',None)
        load(control,args.game,args.dependency,args.cache)
        relative={Path(p).relative_to(args.mod).as_posix().lower() for p,_,_ in target if Path(p).is_relative_to(args.mod)}
        # Reuse the loader's complete categories and original ASTs. Rebuilding
        # common/governments/civics as 'governments' silently skipped its rules
        # whenever --dependency was supplied.
        native_paths={str(p) for rel,p in control.final_files.items() if rel in relative}
        native_target=[record for record in control.file_records if record[0] in native_paths]
        control.mod_files={p for p,_,_ in native_target}
        control.diagnostics=[d for d in control.diagnostics if d['path'] in control.mod_files]
        analyze(control,native_target,args)
        attribute(checker,control,args.game,args.dependency+[args.mod])
        controls={'mode':'fresh_native_counterparts_and_reached_calls','complete_vanilla_semantic_check':False,'selected_files':[p for p,_,_ in native_target],'rules_sha256':hashlib.sha256(args.rules.read_bytes()).hexdigest(),'declarations_sha256':binding(control,args)['declarations_sha256'],'input_manifest':control.input_manifest,'diagnostics':control.diagnostics,'game_version':version,'dependencies':[str(p) for p in args.dependency]}
    errors=sum(d['severity'] in {'error','fatal'} for d in checker.diagnostics)
    warnings=sum(d['severity']=='warning' for d in checker.diagnostics)
    report={'tool':'Tiger Stellar','tool_version':VERSION,'game_version':version,'rules_sha256':hashlib.sha256(args.rules.read_bytes()).hexdigest(),'upstream_commit':'9e3d3b25c7524cdcc86d5f4190f3f8cffbc4d630','status':'empty_input_not_validation' if input_state!='script-targets' else ('errors' if errors else ('warnings_require_review' if warnings else 'static_checks_pass')),'input_state':input_state,'full_dsl_validated':False,'validation_mode':'syntax-only' if args.syntax_only else 'bounded-semantic','errors':errors,'warnings':warnings,'syntax_files':checker.syntax_files,'template_files':checker.template_files,'target_files':len(target),'checked_statements':dict(checker.checked),'coverage_gaps':dict(checker.gaps),'uninvoked_mod_scripts':checker.uninvoked_mod_scripts,'inference_iterations':inference_iterations,'global_target_types':checker.global_targets,'project_location_types':checker.project_locations,'inferred_script_inputs':checker.inferred_script_inputs,'scope_free_event_root_policy':'unknown ROOT; THIS remains none; engine contract not verified','numeric_variable_types':[{'scope_type':s,'name':n,'type':t,'availability_proven':False} for (s,n),t in sorted(checker.variable_types.items())],'expansion_trace':checker.expansion_trace,'engine_not_run':True,'elapsed_seconds':round(time.monotonic()-start,2),'diagnostics':sorted(checker.diagnostics,key=lambda d:(d['path'],d['line'],d['column'],d['key'],d['message'],d['severity'])),'baseline_parse_diagnostics':checker.vanilla_parse_diagnostics,'input_manifest':checker.input_manifest,'rules_provenance':rules['sources']}
    report.update(binding=binding(checker,args),native_control=controls,engine_entries=checker.engine_entries,ship_feasibility=getattr(checker,'ship_feasibility',[]),coverage_summary=coverage_summary(checker),checked_statements_scope='counts of visits in target files and reached expansions, not unique source coverage and not a percentage',selected_language=getattr(args,'language',None),localisation_duplicates=getattr(checker,'localisation_duplicates',[]))
    report.update(project_constraints=policy,overrides_report=str(args.overrides_output) if getattr(args,'overrides_output',None) else None)
    report.update(context_flow=checker.flow_report(),component_potential_proofs=getattr(checker,'potential_proofs',[]))
    from diagnostic_kinds import summary
    from diagnostic_kinds import classify
    for diagnostic in checker.vanilla_parse_diagnostics:
        if 'reason' not in diagnostic:classify(diagnostic)
    if controls:controls['warning_summary']=summary(controls['diagnostics'])
    report.update(diagnostic_schema_version=2,warning_summary=summary(report['diagnostics']),diagnostic_reason_policy='Reason class describes checker evidence; no severity changes or suppression; unresolved information is not proof of runtime validity.')
    report['warning_summary_scope']='Unique target/reached warning diagnostics only; excludes separately recorded baseline parse diagnostics; not statement visits or coverage percentage.'
    if getattr(args,'compare_report',None):report['diagnostic_delta']=diagnostic_delta(report,json.loads(args.compare_report.read_text(encoding='utf8')))
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf8')
    print(json.dumps({k:report[k] for k in ['tool','status','errors','warnings','syntax_files','target_files','elapsed_seconds']},ensure_ascii=False))
    return 1 if errors else (2 if warnings else 0)

def main():
    p=argparse.ArgumentParser(description='Tiger Stellar - Stellaris static checker (engine-schema coverage is reported explicitly)')
    p.add_argument('--version',action='version',version='Tiger Stellar '+VERSION)
    p.add_argument('--game',type=Path,required=True);p.add_argument('--mod',type=Path);p.add_argument('--dependency',type=Path,action='append',default=[])
    p.add_argument('--syntax-only',action='store_true',help='parse and index without running semantic checks; never claims full DSL validation')
    p.add_argument('--native-control',action='store_true',help='fresh check of native counterpart target files and their reached calls; no full vanilla semantic scan')
    p.add_argument('--language',help='require referenced localization keys in this language, e.g. simp_chinese')
    p.add_argument('--compare-report',type=Path,help='compare diagnostics only if input paths and SHA256 sets match')
    p.add_argument('--overrides-output',type=Path,help='export layered file diffs and effective definition providers')
    p.add_argument('--project-constraints',type=Path,help='optional project-specific constraints JSON; no policy is imposed by default')
    p.add_argument('--rules',type=Path,default=resource_root()/'rules/stellaris-4.5.2.json');p.add_argument('--output',type=Path,required=True);p.add_argument('--cache',type=Path)
    args=p.parse_args()
    try:return run(args)
    except Exception as e:
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps({'tool':'Tiger Stellar','status':'infrastructure_error','errors':1,'full_dsl_validated':False,'message':str(e)},ensure_ascii=False,indent=2),encoding='utf8');print(str(e),file=sys.stderr);return 3

if __name__=='__main__':sys.exit(main())
