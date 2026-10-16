"""Stable diagnostic reasons. Classification never changes severity or filters.

A reason identifies what the checker knows, not whether the game code is good.
Legacy message mappings are explicitly identified; new call sites may supply
structured evidence so consumers need not parse English messages.
"""
import collections,hashlib,json

KINDS={
 'rule-unimplemented':('规则或子语法未实现','checker-maintainer','实现字段、结构与作用域合同，并添加合法/非法样例。'),
 'engine-contract-unverified':('引擎合同尚未证实','engine-contract-research','核对引擎导出和原版说明；仍不明确时进行针对性引擎验证。'),
 'caller-context-unknown':('脚本调用入口未知','caller-analysis','追踪脚本调用者或可证实的引擎入口，不凭脚本名字猜作用域。'),
 'scope-type-unknown':('作用域类型无法推导','scope-analysis','查看调用链与 THIS/ROOT/FROM/PREV 证据，补足缺失的类型来源。'),
 'scope-type-ambiguous':('作用域存在多个候选类型','scope-analysis','核对条件分支及类型收窄，逐个验证可达的候选上下文。'),
 'dynamic-target-identity':('动态目标身份无法确定','target-analysis','核对目标保存点、对象选择器与调用传播；类型事实不等于运行期目标存在。'),
 'numeric-information-unresolved':('数值或变量信息不足','value-analysis','核对赋值、变量类型与表达式合同；未知值不代表变量不存在。'),
 'component-eligibility-unresolved':('组件条件尚未得到完整证明','condition-analysis','检查船体与组件的 potential、国家前提和证明边界；不能据此判定自动设计可用。'),
 'dynamic-reference':('动态引用无法静态解析','reference-analysis','核对参数展开与目标来源；需要时以具体实例验证引用。'),
 'analysis-limit':('分析达到边界','checker-maintainer','检查递归、固定点或深度限制；结果属于未完成分析。'),
 'graphics-semantics-unimplemented':('图形行为或材质语义未实现','graphics-validation','资源引用检查不能证明渲染效果；补语义规则或进行图形验收。'),
 'formatting-suspicion':('括号位置格式怀疑','project-review','按配对位置人工复核缩进；这条启发式提示不是确定的语法错误。'),
 'localisation-review':('本地化引用需复核','project-review','检查所选语言、加载顺序和本地化键。'),
 'input-configuration':('输入或游戏版本需复核','input-review','检查游戏版本和真实目标目录；零目标不能作为验收通过。'),
 'unclassified-review':('尚未归类的告警','checker-maintainer','保留原始诊断并补充原因合同，不能据此判定代码正确或错误。'),
 'confirmed-rule-violation':('违反已实现的静态规则','project-review','核对原始错误、规则依据和原版对照；引擎缺陷是否成立仍需独立证据。'),
}

def infer_kind(key,message,severity):
 if severity in {'error','fatal'}:return 'confirmed-rule-violation'
 if key=='brace-placement':return 'formatting-suspicion'
 if key=='localization':return 'localisation-review'
 if key!='validation-coverage':
  if 'version' in key or 'empty' in key or 'version' in message.lower() or 'zero target' in message.lower():return 'input-configuration'
  return 'unclassified-review'
 m=message.lower()
 if any(x in m for x in ['did not converge','recursive ','symbol limit','depth limit']):return 'analysis-limit'
 if 'no inferred caller context' in m:return 'caller-context-unknown'
 if m.startswith('polymorphic scope'):return 'scope-type-ambiguous'
 if 'dynamic event target identity' in m:return 'dynamic-target-identity'
 if m.startswith('picture_event_data selector contract:') and m.endswith('/?'):return 'scope-type-unknown'
 if 'unresolved component eligibility' in m:return 'component-eligibility-unresolved'
 if any(x in m for x in ['fixed/generated design bypass','engine contract','not documented','selector contract','literal selector']):return 'engine-contract-unverified'
 if 'graphics behavior/shader/material' in m or 'gui behavioral' in m:return 'graphics-semantics-unimplemented'
 if 'scope cannot be inferred' in m or 'context cannot be inferred' in m:return 'scope-type-unknown'
 if 'numeric' in m or 'bounds cannot be ordered' in m or 'script value result' in m:return 'numeric-information-unresolved'
 if 'dynamic entity reference' in m or 'dynamic resource path' in m:return 'dynamic-reference'
 if any(x in m for x in ['grammar','subgrammar','undocumented parameter','definition field','metadata','field:','block-valued','special token','declaration form']):return 'rule-unimplemented'
 return 'unclassified-review'

def classify(record,kind=None,evidence=None):
 code=kind or infer_kind(record['key'],record['message'],record['severity'])
 if code not in KINDS:raise ValueError('unknown diagnostic reason '+code)
 title,owner,action=KINDS[code]
 record['reason']={'code':code,'title_zh':title,'responsibility':owner,'next_action_zh':action,'classification_source':'explicit-contract' if kind else 'legacy-message-mapping','static_rule_violation':record['severity'] in {'error','fatal'},'game_code_fault_proven':False,'engine_fault_proven':False,'evidence':evidence or {}}
 record['diagnostic_id']=hashlib.sha256(json.dumps([record.get(k) for k in ['path','line','column','key','message','severity']],ensure_ascii=False).encode()).hexdigest()[:20]

def summary(diagnostics):
 groups={}
 for d in diagnostics:
  if 'reason' not in d:classify(d)
  if d['severity']!='warning':continue
  code=d['reason']['code'];g=groups.setdefault(code,{'code':code,'title_zh':d['reason']['title_zh'],'count':0,'next_action_zh':d['reason']['next_action_zh'],'responsibility':d['reason']['responsibility'],'representative_positions':[],'affected_definitions':set()})
  g['count']+=1;g['affected_definitions'].add(d.get('definition') or '<unknown>')
  if len(g['representative_positions'])<3:g['representative_positions'].append({k:d.get(k) for k in ['path','line','key','message','diagnostic_id']})
 for g in groups.values():g['affected_definitions']=sorted(g['affected_definitions'])
 return sorted(groups.values(),key=lambda g:(-g['count'],g['code']))

