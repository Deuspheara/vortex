#!/usr/bin/env python3
"""A deterministic app-server peer. Never invokes Codex or network services."""
import json,sys,pathlib
root=pathlib.Path(__file__).parent
history=root/'history.json'
turns=json.loads(history.read_text()) if history.exists() else []
active=None
parallel_pending=set()

def send(value):
 print(json.dumps(value),flush=True)
def reply(req,result):send({'id':req['id'],'result':result})
def notice(method,params):send({'method':method,'params':params})
def finish(text='hello world',status='completed'):
 global active
 item={'id':'message-'+active['id'],'type':'agentMessage','text':text}
 active['items']=[item];active['status']=status
 notice('item/completed',{'threadId':'thread-1','turnId':active['id'],'item':item})
 notice('turn/completed',{'threadId':'thread-1','turn':active})
 history.write_text(json.dumps(turns))
for line in sys.stdin:
 req=json.loads(line);method=req.get('method');params=req.get('params',{})
 if not method:
  (root/'decision.json').write_text(json.dumps(req))
  if req.get('id') in parallel_pending:
   parallel_pending.remove(req['id'])
   if not parallel_pending:finish('both approved')
  elif req.get('result',{}).get('decision')=='accept':finish('approved')
  else:finish('denied')
 elif method=='initialize':reply(req,{'userAgent':'fake'})
 elif method=='initialized':pass
 elif method=='account/read':reply(req,{'account':None if (root/'signed-out').exists() else {'type':'chatgpt'},'requiresOpenaiAuth':True})
 elif method=='model/list':reply(req,{'data':[{'model':'test-model','defaultReasoningEffort':'medium','supportedReasoningEfforts':[{'reasoningEffort':'medium'}]}],'nextCursor':None})
 elif method=='thread/start':
  (root/'starts').write_text('started')
  reply(req,{'thread':{'id':'thread-1'}})
 elif method=='thread/resume':
  (root/'resumes').write_text('resumed')
  reply(req,{'thread':{'id':'thread-1','turns':turns}})
 elif method=='thread/turns/list':reply(req,{'data':turns,'nextCursor':None})
 elif method=='turn/start':
  assert params['sandboxPolicy']=={'type':'readOnly','networkAccess':False}
  active={'id':'turn-'+str(len(turns)+1),'status':'inProgress','items':[]};turns.append(active)
  reply(req,{'turn':active})
  notice('turn/started',{'threadId':'thread-1','turn':active})
  text=params['input'][0]['text']
  if text=='approval':
   notice('item/started',{'threadId':'thread-1','turnId':active['id'],'item':{'id':'cmd','type':'commandExecution','command':'echo test'}})
   send({'id':'approval-1','method':'item/commandExecution/requestApproval','params':{'threadId':'thread-1','turnId':active['id'],'itemId':'cmd','command':'echo test'}})
  elif text=='parallel-approval':
   parallel_pending={'parallel-1','parallel-2'}
   for n in (1,2):
    notice('item/started',{'threadId':'thread-1','turnId':active['id'],'item':{'id':'cmd-'+str(n),'type':'commandExecution','command':'echo '+str(n)}})
    send({'id':'parallel-'+str(n),'method':'item/commandExecution/requestApproval','params':{'threadId':'thread-1','turnId':active['id'],'itemId':'cmd-'+str(n),'command':'echo '+str(n)}})
  elif text in ('ui-thinking','ui-style','ui-stream'):
   # Native transcript QA: protocol events only; no command or file is executed.
   notice('item/reasoning/summaryTextDelta',{'threadId':'thread-1','turnId':active['id'],'itemId':'ui-reasoning','summaryIndex':0,'delta':'**Checking the transcript layout**\nCompare the icon column, typography, command output, and code preview at narrow and wide widths.'})
   if text=='ui-stream':
    notice('item/agentMessage/delta',{'threadId':'thread-1','turnId':active['id'],'itemId':'message-'+active['id'],'delta':'A streaming code preview.\n\n```rust\nfn main() {\n    println!("alpha");\n    println!("STREAM-TAIL-LINE");\n}'})
   if text=='ui-style':
    cmd={'id':'ui-command','type':'commandExecution','command':"printf 'alpha\\nbeta\\n'",'status':'inProgress'}
    notice('item/started',{'threadId':'thread-1','turnId':active['id'],'item':cmd})
    notice('item/commandExecution/outputDelta',{'threadId':'thread-1','turnId':active['id'],'itemId':'ui-command','delta':'alpha\nbeta\n'})
    cmd.update(status='completed',exitCode=0,aggregatedOutput='alpha\nbeta\n')
    notice('item/completed',{'threadId':'thread-1','turnId':active['id'],'item':cmd})
    finish('The command output stays in the activity log.\n\n## Code preview\n\n```rust\nfn main() {\n    println!("alpha");\n    println!("beta");\n}\n```\n\nTHREAD-STYLE-END')
  elif text=='wait':pass
  elif text=='unknown-request':
   send({'id':'unknown-1','method':'item/permissions/requestApproval','params':{'threadId':'thread-1','turnId':active['id']}})
  elif text=='crash' and not (root/'crashed').exists():
   notice('item/agentMessage/delta',{'threadId':'thread-1','turnId':active['id'],'itemId':'message-'+active['id'],'delta':'hello'})
   active['status']='completed';active['items']=[{'id':'message-'+active['id'],'type':'agentMessage','text':'hello world'}]
   history.write_text(json.dumps(turns));(root/'crashed').touch();sys.exit(0)
  else:
   notice('item/agentMessage/delta',{'threadId':'thread-1','turnId':active['id'],'itemId':'message-'+active['id'],'delta':'hello'})
   finish()
 elif method=='turn/interrupt':
  reply(req,{})
  finish('interrupted','interrupted')
 else:send({'id':req['id'],'error':{'code':-32601,'message':'unsupported'}})
