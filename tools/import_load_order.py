"""Export a reviewable Tiger invocation from dlc_load.json; never edit launcher data."""
import argparse,hashlib,json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'src'))
from native_parser import parse_text

def export(load_list,user_data):
    obj=json.loads(load_list.read_text(encoding='utf-8-sig'));names=obj['enabled_mods']
    if not isinstance(names,list) or any(not isinstance(x,str) for x in names):raise ValueError('enabled_mods must be an ordered string list')
    resolved=[]
    for name in names:
        descriptor=(user_data/name).resolve() if not Path(name).is_absolute() else Path(name).resolve()
        if not descriptor.is_file():raise ValueError('missing enabled mod descriptor: '+str(descriptor))
        nodes=parse_text(descriptor.read_text(encoding='utf-8-sig'),str(descriptor))['ast'];fields={n.get('key'):n.get('value') for n in nodes}
        if 'archive' in fields:raise ValueError('archive dependency must be explicitly extracted first: '+str(descriptor))
        raw=fields.get('path')
        if not raw:raise ValueError('descriptor has no path: '+str(descriptor))
        root=Path(raw);root=root.resolve() if root.is_absolute() else (user_data/root).resolve()
        if not root.is_dir():raise ValueError('resolved mod directory is missing: '+str(root))
        resolved.append({'descriptor':str(descriptor),'descriptor_sha256':hashlib.sha256(descriptor.read_bytes()).hexdigest(),'directory':str(root),'name':fields.get('name')})
    args=[]
    for i,r in enumerate(resolved):args.extend(['--mod' if i==len(resolved)-1 else '--dependency',r['directory']])
    return {'source':str(load_list),'source_sha256':hashlib.sha256(load_list.read_bytes()).hexdigest(),'load_order':resolved,'arguments':args,'target':'last enabled mod; review before use','launcher_modified':False}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--load-list',type=Path,required=True);p.add_argument('--user-data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
    data=export(a.load_list,a.user_data);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(data,ensure_ascii=False,indent=2),encoding='utf8')
