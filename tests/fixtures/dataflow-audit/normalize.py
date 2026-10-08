import json,re,sys,html
from pathlib import Path
prefix=sys.argv[1]
original=[json.loads(s) for s in re.findall("FLOW_JSON_START\n(.*?)FLOW_JSON_END",Path(prefix+"-stdout.log").read_text(),re.S)]
Path(prefix+"-original.json").write_text(json.dumps(original,indent=2))
contexts=[json.loads(s) for s in re.findall("CONTEXT_JSON_START\\n(.*?)CONTEXT_JSON_END",Path(prefix+"-stdout.log").read_text(),re.S)]
Path(prefix+"-context.json").write_text(json.dumps(contexts[0] if contexts else [],indent=2))
internal=[]
methods=[]
for raw in original:
 if raw['name']=='<global>': continue
 f=json.loads(json.dumps(raw))
 ids={n['id']:i for i,n in enumerate(f['cpg']['nodes'])}
 entry=next(n['id'] for n in raw['cpg']['nodes'] if n['kind']=='METHOD')
 nodes={n['id']:n for n in raw['cpg']['nodes']}
 if any(e['kind']=='CFG' and e['source']==entry and nodes[e['target']]['kind']!='METHOD_RETURN' for e in raw['cpg']['edges']): internal.append(f['fullname'])
 for n in f['cpg']['nodes']:
  n['id']=ids[n['id']]
  if n['kind']=='FIELD_IDENTIFIER': n['name']=n.get('canonical_name',n.get('name'))
  for k in ['argument_index','order','canonical_name']: n.pop(k,None)
  for k in ['name','type_name','method_full_name']:
   if not n.get(k): n.pop(k,None)
 for e in f['cpg']['edges']:
  e['source']=ids[e['source']];e['target']=ids[e['target']]
  if e['kind'] not in ('ARGUMENT','REACHING_DEF'):e.pop('label',None)
 for d in f['reaching_definitions']:
  d['node']=ids[d['node']]
  for k in ('incoming','outgoing'):d[k]=[ids[i] for i in d[k]]
 # Parse original serialized DOT, preserving original projected node/edge identities.
 dot=raw['dot_ddg'][0]
 vertex_ids=set(int(i) for i in re.findall(r'^"([0-9]+)" \[label',dot,re.M))
 by_id={n['id']:n for n in f['cpg']['nodes']}
 f['ddg_view']={'nodes':[by_id[ids[i]] for i in sorted(vertex_ids)],
   'edges':[{'source':ids[int(a)],'target':ids[int(b)],'kind':'DDG','label':html.unescape(label or '')}
       for a,b,label in re.findall(r'"([0-9]+)"\s*->\s*"([0-9]+)"[ \t]*(?:\[ label = "([^"]*)"\])?',dot)]}
 f['filename']=Path(f['filename']).name
 for k in ('capture_nodes','dot_ddg','external_ddg_edges','callee_context','is_external'): f.pop(k,None)
 methods.append(f)
filenames={f['filename'] for f in methods}
all_internal=[m['fullname'] for m in contexts[0] if not m['is_external']] if contexts else [f['fullname'] for f in original]
if contexts: internal=[m['fullname'] for m in contexts[0] if not m['is_external'] and not m['is_stub']]
Path(prefix+"-normalized.json").write_text(json.dumps({'generator':'Original Joern 4.0.150 live audit','internal_methods':{f:internal for f in filenames},'all_internal_methods':{f:all_internal for f in filenames},'methods':methods},indent=2))
print(prefix,len(methods),'methods normalized,',len(internal),'internal')
