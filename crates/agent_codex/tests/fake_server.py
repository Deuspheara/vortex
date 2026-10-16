#!/usr/bin/env python3
"""A deterministic app-server peer. Never invokes Codex or network services."""
import json,sys,pathlib
root=pathlib.Path(__file__).parent
history=root/'history.json'
turns=json.loads(history.read_text()) if history.exists() else []
active=None

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
  if req.get('result',{}).get('decision')=='accept':finish('approved')
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
