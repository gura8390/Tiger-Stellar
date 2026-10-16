"""Finite Boolean proof of component POTENTIAL coverage, not runtime design.

Hull identity/class and scalar scripted predicates are interpreted. Stable
unknown predicates are symbolic, so A or NOT A can be proved without knowing A.
Volatile/unsupported constructs receive distinct symbols and cannot manufacture
a complement proof. Technology unlocks and runtime availability are not proved.
"""
import itertools,json

def neg(x):
 if isinstance(x,bool):return not x
 return x[1] if x[0]=='not' else ('not',x)

def combine(kind,parts):
 flat=[]
 for p in parts:
  if isinstance(p,bool):
   if p==(kind=='or'):return p
   continue
  if p[0]==kind:flat.extend(p[1:])
  else:flat.append(p)
 distinct=set(flat)
 if any(neg(p) in distinct for p in distinct):return kind=='or'
 if not distinct:return kind=='and'
 if len(distinct)==1:return next(iter(distinct))
 return (kind,*sorted(distinct,key=repr))

class PotentialProof:
 def __init__(self,checker,hull):
  self.checker=checker;self.hull=hull;self.serial=0;self.script_cache={}
 def atom(self,node,path,stable=True):
  from reporting import canonical
  n=canonical(node);value=n.get('value');inverse=value=='no'
  if value in {'yes','no'}:n={**n,'value':'yes'}
  identity=json.dumps([path,n],sort_keys=True,ensure_ascii=False)
  if not stable:self.serial+=1;identity+='@occurrence:'+str(self.serial)
  x=('atom',identity)
  return neg(x) if inverse else x
 def body(self,nodes,path=(),active=()):return combine('and',[self.node(n,path,active) for n in nodes])
 def node(self,n,path,active):
  from stellar import children,fields
  k=n.get('key','');low=k.lower();v=n.get('value')
  if low=='always' and v in {'yes','no'}:return v=='yes'
  if not path and low=='is_ship_size' and v is not None:return self.hull==v
  if not path and low=='is_ship_class':
   h=self.checker.definitions.get('ship_sizes',{}).get(self.hull,{})
   known=fields(h).get('class',{}).get('value')
   if known is not None:return known==v
  if low in {'and','or','not','nor','nand','hidden_trigger'}:
   kind='or' if low in {'or','nor'} else 'and';x=combine(kind,[self.node(c,path,active) for c in children(n)])
   return neg(x) if low in {'not','nor','nand'} else x
  scripts=self.checker.scripts['triggers']
  if k in scripts and v in {'yes','no'} and k not in active:
   definition,_=scripts[k]
   if 'macro_parts' not in definition:
    key=(k,path)
    if key not in self.script_cache:self.script_cache[key]=self.body(children(definition),path,active+(k,))
    x=self.script_cache[key];return neg(x) if v=='no' else x
  if k in self.checker.rules['scopes'] and children(n):
   # A transition may select no object. owner={A} OR owner={NOT A}
   # does not prove that an owner exists.
   # Component potential has a preset FROM country (same contract as the
   # semantic traversal); other transitions may fail to select an object.
   exists=True if path==() and k.lower()=='from' else ('atom',json.dumps(['scope-exists',path+(k,)]))
   return combine('and',[exists,self.body(children(n),path+(k,),active)])
  # Only known scalar trigger predicates are stable propositions. Unknown
  # blocks, recursive scripts and random predicates must not share identities.
  stable=k in self.checker.rules['triggers'] and not children(n) and 'random' not in low
  return self.atom(n,path,stable)
 def prove(self,candidates,limit=12):
  from stellar import fields,children
  expression=combine('or',[self.body(children(fields(c).get('potential',{}))) for c in candidates])
  hull=self.checker.definitions.get('ship_sizes',{}).get(self.hull,{})
  assumption=self.body(children(fields(hull).get('potential_country',{})),('from',))
  symbols=set()
  def collect(x):
   if isinstance(x,bool):return
   if x[0]=='atom':symbols.add(x[1])
   else:
    for child in x[1:]:collect(child)
  collect(expression)
  collect(assumption)
  def evaluate(x,assignment):
   if isinstance(x,bool):return x
   if x[0]=='atom':return assignment[x[1]]
   if x[0]=='not':return not evaluate(x[1],assignment)
   return (any if x[0]=='or' else all)(evaluate(c,assignment) for c in x[1:])
  if len(symbols)>limit:return {'status':'unknown','reason':'symbol limit','symbols':len(symbols),'proof_domain':'component potential only'}
  outcomes=set();counterexample=None
  for values in itertools.product([False,True],repeat=len(symbols)):
   assignment=dict(zip(sorted(symbols),values))
   if not evaluate(assumption,assignment):continue
   answer=evaluate(expression,assignment);outcomes.add(answer)
   if not answer and counterexample is None:counterexample=assignment
   if len(outcomes)==2:break
  return {'status':'covered' if outcomes=={True} else 'impossible' if outcomes=={False} else 'unknown','symbols':len(symbols),'counterexample':counterexample,'assumption':'hull potential_country holds for component FROM country','proof_domain':'component potential only; technology, resources and runtime availability not proved'}
