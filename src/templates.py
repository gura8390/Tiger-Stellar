"""Stellaris inline-template expansion before native syntax parsing."""
import re

def render(text,args):
    def walk(pos,closing=False):
        out=[];quoted=False;comment=False
        while pos<len(text):
            c=text[pos]
            if comment:
                out.append(c);pos+=1
                if c=='\n':comment=False
                continue
            if c=='"' and (pos==0 or text[pos-1]!='\\'):quoted=not quoted
            if not quoted and c=='#':comment=True
            if not quoted and text.startswith('[[',pos):
                end=text.find(']',pos+2)
                if end<0:raise ValueError('unterminated conditional header')
                condition=text[pos+2:end].strip();body,endpos=walk(end+1,True)
                active=(condition.lstrip('!') in args) != condition.startswith('!')
                header=text[pos:end+1]
                out.append(re.sub(r'[^\n]',' ',header)+(body if active else re.sub(r'[^\n]',' ',body))+' ')
                pos=endpos;continue
            if not quoted and c=='@' and (text.startswith('@[',pos) or text.startswith('@\\[',pos)):
                end=pos;depth=0
                while end<len(text):
                    if text[end]=='[':depth+=1
                    if text[end]==']':
                        depth-=1
                        if depth==0:break
                    end+=1
                if end>=len(text):raise ValueError('unterminated reader expression')
                out.append(text[pos:end+1]);pos=end+1;continue
            if not quoted and c==']' and closing:return ''.join(out),pos+1
            out.append(c);pos+=1
        if closing:raise ValueError('unterminated conditional block')
        return ''.join(out),pos
    active,_=walk(0)
    missing=set()
    def substitute(m):
        name,separator,default=m[1].partition('|')
        if name in args:return str(args[name])
        if separator:return default
        missing.add(name);return m[0]
    # Comments aren't macro invocation sites; strings may contain parameters.
    pattern=r'#[^\n]*|"(?:\\.|[^"\\])*"|\$[^$\r\n]+\$'
    previous=0;result=[]
    for m in re.finditer(pattern,active):
        result.append(active[previous:m.start()]);token=m[0]
        result.append(token if token.startswith('#') else re.sub(r'\$([^$\r\n]+)\$',substitute,token));previous=m.end()
    result.append(active[previous:])
    if missing:raise ValueError('missing template arguments: '+', '.join(sorted(missing)))
    return ''.join(result)
