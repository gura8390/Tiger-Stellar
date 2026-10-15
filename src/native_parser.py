"""Batching bounds the upstream arena's process-lifetime allocations."""
import json, subprocess, sys
from functools import lru_cache
from pathlib import Path

def resource_root():
    return Path(getattr(sys,'_MEIPASS',Path(__file__).resolve().parents[1]))

def parser_path():
    root=resource_root()
    return root/'stellaris-parser.exe' if (root/'stellaris-parser.exe').exists() else root/'dist/stellaris-parser.exe'

def explain_braces(item,text):
    """Explain the lexer's column-one heuristic, preserving every diagnostic."""
    findings={d['line']:d for d in item.get('diagnostics',[]) if d['key']=='brace-placement'}
    if not findings:return item
    stack=[];quoted=False;escaped=False;comment=False;line=1;column=0
    for char in text:
        column+=1
        if char=='\n':line+=1;column=0;comment=False;continue
        if comment:continue
        if quoted:
            if escaped:escaped=False
            elif char=='\\':escaped=True
            elif char=='"':quoted=False
            continue
        if char=='#':comment=True
        elif char=='"':quoted=True
        elif char=='{':stack.append((line,column))
        elif char=='}':
            opener=stack.pop() if stack else None
            if line in findings and column==1 and opener:
                d=findings[line];d['classification']='formatting_suspicion'
                d['message']=f'possible brace error: closing brace at column 1 matches opening brace at {opener[0]}:{opener[1]}; {len(stack)} enclosing block(s) remain. Indentation heuristic, not a confirmed syntax error.'
    return item

def parse_many(paths, batch_size=100, globals_text=''):
    paths=list(paths)
    for offset in range(0,len(paths),batch_size):
        batch=paths[offset:offset+batch_size]
        request=''.join(json.dumps({'path':str(p),'globals':globals_text},ensure_ascii=False)+'\n' for p in batch)
        p=subprocess.run([str(parser_path())],input=request,capture_output=True,encoding='utf8',creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
        if p.returncode: raise RuntimeError('Tiger parser failed: '+p.stderr[-2000:])
        lines=p.stdout.splitlines()
        if len(lines)!=len(batch): raise RuntimeError('Tiger parser lost a response: '+p.stderr[-2000:])
        for line in lines:
            item=json.loads(line)
            if 'fatal' in item: raise RuntimeError(item['fatal'])
            yield explain_braces(item,Path(item['path']).read_text(encoding='utf-8-sig'))

@lru_cache(maxsize=256)
def _parse_text_payload(exe,identity,text,path,globals_text):
    p=subprocess.run([exe],input=json.dumps({'path':path,'content':text,'globals':globals_text})+'\n',capture_output=True,encoding='utf8',creationflags=getattr(subprocess,'CREATE_NO_WINDOW',0))
    if p.returncode: raise RuntimeError(p.stderr)
    return p.stdout

def parse_text(text,path='fixture.txt',globals_text=''):
    exe=parser_path();stat=exe.stat()
    # Cache immutable serialized ASTs; each caller receives fresh locations/nodes.
    result=json.loads(_parse_text_payload(str(exe),(stat.st_mtime_ns,stat.st_size),text,path,globals_text))
    if 'fatal' in result:raise RuntimeError(result['fatal'])
    return explain_braces(result,text)
