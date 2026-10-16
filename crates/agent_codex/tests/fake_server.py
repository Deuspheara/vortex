#!/usr/bin/env python3
"""A deterministic app-server peer. Never invokes Codex or network services."""
import json,sys,pathlib,threading,time,base64,struct,zlib
root=pathlib.Path(__file__).parent
history=root/'history.json'
turns=json.loads(history.read_text()) if history.exists() else []
active=None
parallel_pending=set()

send_lock=threading.Lock()
def send(value):
 with send_lock:
  sys.stdout.write(json.dumps(value)+'\n');sys.stdout.flush()
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
  elif req.get('result',{}).get('action')=='cancel':pass
  elif req.get('result',{}).get('decision',req.get('result',{}).get('action'))=='accept':finish('approved')
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
  if text in ('approval','long-approval'):
   approval_command = ('python3 - <<\'PY\'\n' + ''.join(f'print("approval line {n}")\n' for n in range(120)) + '# COMMAND-END\nPY') if text=='long-approval' else 'echo test'
   notice('item/started',{'threadId':'thread-1','turnId':active['id'],'item':{'id':'cmd','type':'commandExecution','command':approval_command}})
   send({'id':'approval-1','method':'item/commandExecution/requestApproval','params':{'threadId':'thread-1','turnId':active['id'],'itemId':'cmd','command':approval_command}})
  elif text=='ui-screen':
   def png_chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
   width,height=640,400
   rows=[]
   for y in range(height):
    row=bytearray()
    for x in range(width):
     color=(40,44,60) if y<40 or x<120 else ((180,145,205) if 160<x<580 and 95<y<140 else (65+(x//40)%2*12,72+(y//40)%2*12,95))
     if x<3 or y<3 or x>=width-3 or y>=height-3:color=(205,215,235)
     row.extend(color)
    rows.append(b'\x00'+bytes(row))
   png=b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))+png_chunk(b'IDAT',zlib.compress(b''.join(rows)))+png_chunk(b'IEND',b'')
   item={'id':'screen-fixture','type':'mcpToolCall','server':'computer-use fixture','tool':'screenshot','status':'completed','result':{'content':[{'type':'text','text':'Simulated capture fixture. No screen was accessed.'},{'type':'image','mimeType':'image/png','data':base64.b64encode(png).decode()}]}}
   notice('item/completed',{'threadId':'thread-1','turnId':active['id'],'item':item})
   finish('Screen preview fixture complete. The image is synthetic; no screen capture was performed.')
  elif text.startswith('elicitation-'):
   schema={'type':'object','properties':{}}
   mode='form'
   if text=='elicitation-boolean':schema['properties']={'screen':{'type':'boolean','title':'Screen access','description':'View the selected screen'}}
   if text=='elicitation-rich':schema['properties']={'password':{'type':'string'}}
   if text=='elicitation-openai':mode='openai/form'
   if text=='elicitation-url':mode='url'
   send({'id':'screen-permission','method':'mcpServer/elicitation/request','params':{'threadId':'thread-1','turnId':None if text=='elicitation-no-turn' else active['id'],'serverName':'computer-use','mode':mode,'message':'Allow access to your screen for this task?','requestedSchema':schema}})
  elif text.startswith('ai-review-'):
   assert params['approvalPolicy']=='on-request'
   assert params['approvalsReviewer']=='auto_review'
   review={'threadId':'thread-1','turnId':active['id'],'reviewId':active['id']+'-review','action':{'type':'command','command':'echo safe-review-fixture'},'review':{'status':'inProgress'}}
   notice('item/autoApprovalReview/started',review)
   if text!='ai-review-wait':
    status={'ai-review-approved':'approved','ai-review-denied':'denied','ai-review-timeout':'timedOut'}.get(text,'aborted')
    review['review']={'status':status,'riskLevel':'low','userAuthorization':'high','rationale':'Deterministic approval-review fixture. No command was executed.'}
    notice('item/autoApprovalReview/completed',review)
    notice('item/autoApprovalReview/completed',review)
    finish('AI review '+status+' (fixture only)')
  elif text=='parallel-approval':
   parallel_pending={'parallel-1','parallel-2'}
   for n in (1,2):
    notice('item/started',{'threadId':'thread-1','turnId':active['id'],'item':{'id':'cmd-'+str(n),'type':'commandExecution','command':'echo '+str(n)}})
    send({'id':'parallel-'+str(n),'method':'item/commandExecution/requestApproval','params':{'threadId':'thread-1','turnId':active['id'],'itemId':'cmd-'+str(n),'command':'echo '+str(n)}})
  elif text in ('ui-thinking','ui-style','ui-stream','ui-thread'):
   # Native transcript QA: protocol events only; no command or file is executed.
   notice('item/reasoning/summaryTextDelta',{'threadId':'thread-1','turnId':active['id'],'itemId':'ui-reasoning','summaryIndex':0,'delta':'**Checking the transcript layout**\nCompare the icon column, typography, command output, and code preview at narrow and wide widths.'})
   if text=='ui-stream':
    notice('item/agentMessage/delta',{'threadId':'thread-1','turnId':active['id'],'itemId':'message-'+active['id'],'delta':'A streaming code preview.\n\n```rust\nfn main() {\n    println!("alpha");\n    println!("STREAM-TAIL-LINE");\n}'})
   if text in ('ui-style','ui-thread'):
    cmd={'id':'ui-command','type':'commandExecution','command':"printf 'alpha\\nbeta\\n'",'status':'inProgress'}
    notice('item/started',{'threadId':'thread-1','turnId':active['id'],'item':cmd})
    notice('item/commandExecution/outputDelta',{'threadId':'thread-1','turnId':active['id'],'itemId':'ui-command','delta':'alpha\nbeta\n'})
    cmd.update(status='completed',exitCode=0,aggregatedOutput='alpha\nbeta\n')
    notice('item/completed',{'threadId':'thread-1','turnId':active['id'],'item':cmd})
    if text=='ui-thread':
     finish('The command printed `alpha` and `beta`.\n\n## Code preview\n\n```rust\nfn main() {\n    println!("alpha");\n    println!("beta");\n    let description = "A deliberately long code line for checking wrapping, horizontal scrolling, and readable type in a narrow transcript without losing the end of the line: CODE-LINE-END";\n}\n```\n\nA second paragraph keeps the same reading size.\n\n- One compact list item\n- Another item with **bold text**\n\n### Small heading\n\nThread typography check complete.')
    else:
     finish('The command output stays in the activity log.\n\n## Code preview\n\n```rust\nfn main() {\n    println!("alpha");\n    println!("beta");\n}\n```\n\nTHREAD-STYLE-END')
  elif text=='diff-updates':
   for value in ('one','two','two'):
    notice('turn/diff/updated',{'threadId':'thread-1','turnId':active['id'],'diff':'--- a/live.txt\n+++ b/live.txt\n@@ -1 +1 @@\n-before\n+'+value+'\n'})
   finish('Updated diff')
  elif text=='ui-live':
   # Protocol-only streaming and observed diffs; no file is actually written.
   def stream_ui(turn):
    chunks=[]
    cmd={'id':'live-command','type':'commandExecution','command':"/bin/zsh -lc \"printf 'alpha\\nbeta\\n'\"",'status':'completed','aggregatedOutput':'alpha\nbeta\n','exitCode':0}
    notice('item/completed',{'threadId':'thread-1','turnId':turn['id'],'item':cmd})
    for n in range(80):
     if turn['status']!='inProgress':return
     chunk=('## Streaming progress\n\n' if n==0 else '')+f'Checkpoint {n}: keep this paragraph stable while new text arrives. The `live.txt` preview updates beside this thread.\n\n'
     chunks.append(chunk)
     notice('item/agentMessage/delta',{'threadId':'thread-1','turnId':turn['id'],'itemId':'message-'+turn['id'],'delta':chunk})
     if n in (2,15,40,65):
      notice('turn/diff/updated',{'threadId':'thread-1','turnId':turn['id'],'diff':'--- /dev/null\n+++ b/live.txt\n@@ -0,0 +1,2 @@\n+Streaming file preview\n+Checkpoint '+str(n)+'\n'})
     time.sleep(0.3)
    if turn['status']=='inProgress':finish(''.join(chunks)+'\nUpdated the component:\n\n- Commands stay quiet with useful details when expanded.\n- Inline `code` has comfortable padding and rounded corners.\n- Wrapped bullet points use the same line height as the surrounding text.\n\n**Streaming check complete.**')
   threading.Thread(target=stream_ui,args=(active,),daemon=True).start()
  elif text=='ui-usage':
   totals={'inputTokens':0,'outputTokens':0,'cachedInputTokens':0,'totalTokens':0}
   for input_tokens,output_tokens,cached in [(19360,95,5632),(19523,29,19200),(19691,2135,5632),(21850,107,19328),(22045,53,19456)]:
    last={'inputTokens':input_tokens,'outputTokens':output_tokens,'cachedInputTokens':cached,'totalTokens':input_tokens+output_tokens}
    for key in totals:totals[key]+=last[key]
    notice('thread/tokenUsage/updated',{'threadId':'thread-1','turnId':active['id'],'tokenUsage':{'total':dict(totals),'last':last,'modelContextWindow':475000}})
   finish('Usage fixture complete: current context 22,098 / 475,000 tokens; processed input 102,469 tokens. No inference or file changes were performed.')
  elif text=='ui-diff':
   # Observed protocol fixture only: no workspace file is changed.
   notice('item/completed',{'threadId':'thread-1','turnId':active['id'],'item':{'id':'ui-file-changes','type':'fileChange','status':'completed','changes':[{'path': 'src/main.rs', 'diff': '@@ -1,3 +1,4 @@\n fn main() {\n-    let message = "old";\n+    let message = "Review layout";\n+    println!("{message}");\n }\n'}, {'path': 'src/config.ts', 'kind': {'type': 'add'}, 'diff': 'export const configuration = {\n  theme: "frappe",\n  description: "A long line for checking readable wrapping and horizontal scrolling in a narrow review panel, without losing content or misaligning the corresponding old and new code lines.",\n  enabled: true,\n' + ''.join(f'  option_{i}: true,\n' for i in range(40)) + '};\n'}, {'path': 'src/legacy.rs', 'kind': {'type': 'delete'}, 'diff': 'pub fn legacy() {}\n// Replaced by the new entry point.\n'}]}})
   finish('Updated the entry point and configuration, and removed the old helper.\n\nReview the three files in Changes. The code view preserves the old and new line numbers, syntax colors, and long lines.')
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
