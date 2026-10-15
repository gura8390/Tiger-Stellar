"""Freeze observed native declaration fields, with source hashes, separately from API contracts.

An observed field is evidence that its name/shape is legal, not an exhaustive grammar.
"""
import argparse, collections, hashlib, json, sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'src'))
from stellar import Checker, load, children, number

def main():
    a=argparse.ArgumentParser();a.add_argument('--game',type=Path,required=True);a.add_argument('--cache',type=Path,required=True);args=a.parse_args()
    root=Path(__file__).resolve().parents[1];c=Checker(json.loads((root/'rules/stellaris-4.5.2.json').read_text(encoding='utf8')))
    target=load(c,args.game,[],args.cache);schema=collections.defaultdict(dict)
    for path,category,ast in target:
        if category in {'gfx','interface','events','scripted_triggers','scripted_effects','script_values','scripted_variables','name_lists'}:continue
        for node in ast:
            for field in children(node):
                k=field.get('key')
                if not k or k.startswith('@'):continue
                entry=schema[category].setdefault(k,{'forms':[],'examples':[],'scalar_types':[]})
                form='block' if 'children' in field else 'scalar'
                if form not in entry['forms']:entry['forms'].append(form)
                if form=='scalar':
                    v=field.get('value','');kind='boolean' if v in {'yes','no'} else ('number' if number(v) is not None else 'string')
                    if kind not in entry['scalar_types']:entry['scalar_types'].append(kind)
                if not entry['examples']:entry['examples'].append({'path':path,'line':field.get('line',0),'sha256':hashlib.sha256(Path(path).read_bytes()).hexdigest()})
    output={'game_id':'stellaris','game_version':c.rules['game_version'],'basis':'observed native fields; nested parameter contracts are validated separately','categories':dict(schema)}
    (root/'rules/declarations-4.5.2.json').write_text(json.dumps(output,ensure_ascii=False,indent=2),encoding='utf8');print('declaration categories',len(schema))
if __name__=='__main__':main()
