"""Generate pinned Stellaris engine API tables; never borrow CK3 scope tables."""
import argparse, hashlib, json, re
from pathlib import Path

def extract(source, game, destination):
    settings=json.loads((game/'launcher-settings.json').read_text(encoding='utf-8-sig'))
    if settings.get('gameId')!='stellaris':raise ValueError('wrong-game: rule extraction requires Stellaris')
    schema = {'schema_version': 1, 'game_id':'stellaris','game_version': settings['rawVersion'], 'sources': {}}
    for kind in ['triggers','effects','scopes']:
        p = source/(kind+'.log'); raw = p.read_bytes(); text = raw.decode('utf-8-sig')
        schema['sources'][kind] = {'path':str(p),'sha256':hashlib.sha256(raw).hexdigest(),'mtime_ns':p.stat().st_mtime_ns}
        entries = {}
        for m in re.finditer(r'^([a-zA-Z_][a-zA-Z_0-9]*) - (.*?)(?=^[a-zA-Z_][a-zA-Z_0-9]* - |\Z)',text,re.M|re.S):
            name, body = m.groups(); supported = re.search(r'^Supported Scopes: (.+)$',body,re.M)
            if not supported: raise ValueError('missing supported scopes: '+name)
            entry = {'input':supported[1].strip().split(),'description':body.strip()}
            out = re.search(r'^Output Scope: (.+)$',body,re.M)
            if out: entry['output'] = out[1].strip()
            if kind=='scopes' and name=='target' and 'spy network to its target country' in body:
                entry['output_by_input']={'spy_network':'country'}
            # Parameter names at depth one in the engine's documented examples.
            depth=0;keys=set();last=0
            for token in re.finditer(r'[{}]|\b([A-Za-z_][A-Za-z_0-9]*(?:/[A-Za-z_][A-Za-z_0-9]*)*)\s*(?:=|[<>])',body):
                if token[0]=='{':depth+=1
                elif token[0]=='}':depth=max(0,depth-1)
                elif depth==1 and token[1]:keys.update(token[1].split('/'))
            if keys:entry['parameters']=sorted(keys)
            # Boolean only when an actual documented assignment says so.
            if re.search(r'\b'+re.escape(name)+r'\s*=\s*(?:<?yes/no>?|yes|no)(?:\s|$)',body): entry['boolean'] = True
            if re.search(r'\b'+re.escape(name)+r'\s*[<>]=?\s*[-\d]',body): entry['numeric'] = True
            # Iterators are identified by engine prose, not merely an any_/count_ prefix.
            if ('Iterate through' in body or 'Iterates through' in body or 'Executes enclosed effects on every system' in body):
                outputs = {'ground_combat_attacker':'army','ground_combat_defender':'army','system_in_cluster':'galactic_object','system_within_border':'galactic_object','owned_pop':'pop','owned_pop_group':'pop_group','relation':'country','neighbor_country':'country','member':'country','pool_leader':'leader','war_participant':'country'}
                stem = re.sub(r'^(any|count|every|random|ordered)_','',name)
                outputs.update({'bypass':'bypass','cosmic_storm':'cosmic_storm','system_within_storm':'galactic_object','cosmic_storm_start_position':'galactic_object','cosmic_storm_end_position':'galactic_object','system_added_to_storm':'galactic_object','system_removed_from_storm':'galactic_object','subject':'country','available_debris':'debris','pre_ftl_within_border':'country','observed_pre_ftl_within_border':'country','owned_design':'design','spynetwork':'spy_network','espionage_asset':'espionage_asset','exhibit':'exhibit','orbital_station':'ship','galcom_member':'country','council_member':'country','envoy':'leader','associate':'country','owned_mission':'mission','issued_mission':'mission','owned_contract':'mission','moon':'planet','system_with_aura':'galactic_object','existing_species_traits':'trait','starbase_in_network':'starbase','neighbor_system_euclidean':'galactic_object','attacker':'country','defender':'country'})
                for suffix,scope in [('cosmic_storm_influence_field','cosmic_storm_influence_field'),('storm_influence_field','cosmic_storm_influence_field'),('ambient_object','ambient_object'),('archaeological_site','archaeological_site'),('espionage_operation','espionage_operation'),('first_contact','first_contact'),('pop_faction','pop_faction'),('pop_group','pop_group'),('astral_rift','astral_rift'),('megastructure','megastructure'),('ship_design','design'),('agreement','agreement'),('situation','situation'),('country','country'),('planet','planet'),('colony','colony'),('system','galactic_object'),('fleet','fleet'),('ship','ship'),('leader','leader'),('army','army'),('species','species'),('deposit','deposit'),('sector','sector'),('pop','pop'),('starbase','starbase'),('federation','federation'),('war','war')]:
                    if stem.endswith(suffix): entry['iterator_output'] = scope; break
                if stem in outputs: entry['iterator_output'] = outputs[stem]
                # Several iterators name their location last (fleet_in_system).
                mobj=re.search(r'Iterate(?:s)? through (?:each|every|all)(?: owned)? (pop groups?|pop factions?|starbases?|fleets?|ships?|arm(?:y|ies)|leaders?|countr(?:y|ies)|planets?|colon(?:y|ies)|species|pops?)\b',body,re.I)
                if mobj and not stem.endswith(('planet','pop')):
                    obj=mobj[1].lower().replace(' ','_')
                    obj={'countries':'country','colonies':'colony','armies':'army','species':'species'}.get(obj,obj.removesuffix('s'))
                    entry['iterator_output']=obj
                if stem.startswith('trait_'):entry['iterator_output']='trait'
            special_outputs={'random_galaxy_species':'species','any_galaxy_species':'species','every_galaxy_species':'species','closest_system':'galactic_object','owned_planet_list_tooltip':'planet','owned_colony_list_tooltip':'colony','count_owned_pop_amount':'pop_group','count_owned_species':'species','count_owned_pop_group':'pop_group'}
            if name in special_outputs:entry['iterator_output']=special_outputs[name]
            if name=='weighted_random_owned_pop_group' and 'owned_pop_group script list' in body:entry['iterator_output']='pop_group'
            if re.sub(r'^(any|count|every|random|ordered)_','',name) in {'owned_species','galaxy_species','enslaved_species'} and ('species' in body):entry['iterator_output']='species'
            entries[name] = entry
        schema[kind] = entries
    p=source/'modifiers.log';raw=p.read_bytes()
    schema['sources']['modifiers']={'path':str(p),'sha256':hashlib.sha256(raw).hexdigest(),'mtime_ns':p.stat().st_mtime_ns}
    schema['modifiers']=sorted(set(re.findall(r'^- ([a-zA-Z_0-9]+), Category:',raw.decode('utf-8-sig'),re.M)))
    # Kept separate from the engine export: its variable frontend is documented
    # by the installed guide and typed current native calling examples.
    native_numeric_sources=['events/000_how_to_use_variables_in_script.txt','events/unrest_events.txt','events/biogenesis_origin_events.txt','events/federations_election_events.txt']
    schema['numeric_expression_sources']=[{'path':str(game/rel),'sha256':hashlib.sha256((game/rel).read_bytes()).hexdigest()} for rel in native_numeric_sources]
    if 'value = trigger:pop_amount' not in (game/native_numeric_sources[0]).read_text(encoding='utf-8-sig'):raise ValueError('native numeric guide changed')
    schema['numeric_expressions']={'pop_amount':{'input':['country','planet','colony','pop_faction'],'numeric':True,'provenance':'installed variable guide (owner.capital_scope) and typed current vanilla callers; absent from engine export'}}
    destination.write_text(json.dumps(schema,ensure_ascii=False,indent=2),encoding='utf8')
    print({k:len(schema[k]) for k in ['triggers','effects','scopes','modifiers']})

if __name__=='__main__':
    a=argparse.ArgumentParser();a.add_argument('--docs',type=Path,required=True);a.add_argument('--game',type=Path,required=True);a.add_argument('--output',type=Path,required=True);ns=a.parse_args();extract(ns.docs,ns.game,ns.output)
