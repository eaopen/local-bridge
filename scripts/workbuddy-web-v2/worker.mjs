// WorkBuddy macOS native account ACP worker, one task only. No tools/MCP.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import {spawn} from 'node:child_process';
import {randomBytes,timingSafeEqual} from 'node:crypto';
import readline from 'node:readline';
import {connectWorkbuddyBroker} from './workbuddyBroker.ts';
import {loadWorkbuddyModelConfig} from './workbuddyModels.ts';

const app='/Applications/WorkBuddy.app/Contents';
const cli=path.join(app,'Resources/app.asar.unpacked/cli');
let incoming='';
for await(const chunk of process.stdin){incoming+=chunk;if(incoming.length>16000)process.exit(2);}
let args;try{args=JSON.parse(incoming);}catch{process.exit(2);}
const {dir,prompt}=args;
if(typeof dir!=='string'||!path.isAbsolute(dir)||typeof prompt!=='string'||
   !prompt.trim()||Buffer.byteLength(prompt)>12000)process.exit(2);
const stat=fs.lstatSync(dir);
if(!stat.isDirectory()||stat.isSymbolicLink()||
   (process.getuid&&stat.uid!==process.getuid())||(stat.mode&0o077)!==0)process.exit(2);
const events=path.join(dir,'events.jsonl');
let seq=0,size=0,current='',msgId=null,messages=0,denied=false;
function emit(type,body={}){
 const row=JSON.stringify({seq:++seq,type,at:Date.now(),...body})+'\n';
 size+=Buffer.byteLength(row);
 if(seq>250||size>512000)throw Error('event_limit');
 fs.appendFileSync(events,row,{flag:'a',mode:0o600});
}
function closeMessage(){if(msgId){emit('message.final',{message_id:msgId});msgId=null;current='';}}
function update(u){
 if(!u||typeof u!=='object')return;
 if(u.sessionUpdate==='agent_message_chunk'&&u.content?.type==='text'){
  const delta=u.content.text;
  if(typeof delta!=='string'||!delta)return;
  if(!msgId)msgId='msg-'+(++messages);
  for(let i=0;i<delta.length;i+=1024){
   const chunk=delta.slice(i,i+1024);
   current+=chunk;emit('message.delta',{message_id:msgId,text:chunk});
  }
 }else if(u.sessionUpdate==='agent_message'&&u.content?.type==='text'){
  const full=String(u.content.text||'');
  if(full!==current&&full){
   closeMessage();msgId='msg-'+(++messages);
   // A snapshot is never appended to previous chunks; it replaces its own message only.
   for(let i=0;i<full.length;i+=1024)emit('message.delta',{message_id:msgId,text:full.slice(i,i+1024)});
  }
  closeMessage();
 }else if(['tool_call','tool_call_update'].includes(u.sessionUpdate)){
  closeMessage();emit('tool.update',{state:String(u.status||'unknown').slice(0,24)});
 }
}
let broker,server,proc,pending=new Map(),counter=0,runtimeFailure;
async function task(){
 emit('task.started',{agent:'workbuddy'});
 if(process.platform!=='darwin'||!fs.existsSync(path.join(cli,'dist/codebuddy-lite-wb.mjs')))
  throw Error('workbuddy_not_installed');
 const config=path.join(dir,'config'),work=path.join(dir,'work');
 fs.mkdirSync(config,{mode:0o700});fs.mkdirSync(work,{mode:0o700});
 broker=await connectWorkbuddyBroker(path.join(os.homedir(),'.workbuddy'));
 const models=await loadWorkbuddyModelConfig(broker,
  JSON.parse(fs.readFileSync(path.join(cli,'product.json'),'utf8')));
 const nonce=randomBytes(32).toString('hex');
 server=http.createServer(async(req,res)=>{
  try{
   const a=Buffer.from(req.headers.authorization||''),b=Buffer.from('Bearer '+nonce);
   if(req.headers.origin||a.length!==b.length||!timingSafeEqual(a,b)){
    res.writeHead(401);return res.end();
   }
   const u=new URL(req.url,'http://127.0.0.1');
   if(!/^\/[A-Za-z0-9_./-]*$/.test(u.pathname)||req.url.startsWith('//'))throw Error('invalid_path');
   let bytes=0,parts=[];
   for await(const c of req){bytes+=c.length;if(bytes>640000)throw Error('too_large');parts.push(c);}
   const headers={};for(const k of ['content-type','accept'])
    if(typeof req.headers[k]==='string')headers[k]=req.headers[k];
   const answer=await broker.requestFetch({method:req.method,path:u.pathname,
     query:Object.fromEntries(u.searchParams),headers,...(bytes?{body_b64:Buffer.concat(parts).toString('base64')}:{})});
   const response=Buffer.from(answer.body_b64||'','base64');
   if(response.length>640000||(answer.status>=300&&answer.status<400))throw Error('bad_reply');
   res.writeHead(answer.status,{'content-type':answer.headers?.['content-type']||'application/json'});res.end(response);
  }catch{
   res.writeHead(502,{'content-type':'application/json'});
   res.end('{"error":{"message":"WorkBuddy unavailable"}}');
  }
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const origin='http://127.0.0.1:'+server.address().port;
 const env={...process.env};
 for(const k of Object.keys(env))if(/^(CODEBUDDY_|WORKBUDDY_|ACC_PRODUCT_CONFIG|ACC_USER_|ANTHROPIC_|OPENAI_)/.test(k))delete env[k];
 Object.assign(env,{
  ELECTRON_RUN_AS_NODE:'1',CODEBUDDY_FORCE_HEADLESS_BUNDLE:'1',
  CODEBUDDY_CONFIG_DIR:config,WORKBUDDY_CONFIG_DIR:config,CODEBUDDY_AUTH_TOKEN:nonce,
  CODEBUDDY_BASE_URL:origin+'/v2',CODEBUDDY_INTERNET_ENVIRONMENT:'external',
  CODEBUDDY_CREDENTIALS_IN_MEMORY:'1',CODEBUDDY_DISABLE_LOCAL_STORAGE:'1',
  CODEBUDDY_DISABLE_PRODUCT_CACHE:'1',CODEBUDDY_DISABLE_CUSTOM_MODELS_FILE:'1',
  CODEBUDDY_API_KEY_DISABLED:'1',CODEBUDDY_API_KEY_HELPER_DISABLED:'1',
  CODEBUDDY_MAX_RETRIES:'0',CODEBUDDY_RETRY_WATCHDOG:'0',
  CODEBUDDY_GATEWAY_AUTH:'password',CODEBUDDY_GATEWAY_PASSWORD:randomBytes(32).toString('hex'),
  CODEBUDDY_GATEWAY_DISABLE_API_DOCS:'1',DISABLE_AUTOUPDATER:'1',DISABLE_TELEMETRY:'1',
  ACC_PRODUCT_CONFIG_V3:JSON.stringify({...models,endpoint:origin,networkEnvironment:'external',
    authentication:{type:'custom-token',attributes:{tokenType:'bearerToken',token:nonce}}})
 });
 proc=spawn(path.join(app,'MacOS/Electron'),
  [path.join(cli,'dist/codebuddy-lite-wb.mjs'),'--serve','--port','0','--host','127.0.0.1',
   '--acp','--acp-transport','stdio','--setting-sources','none','--strict-mcp-config',
   '--mcp-config','{"mcpServers":{}}','--tools','','--no-session-persistence'],
  {cwd:work,env,stdio:['pipe','pipe','pipe']});
 proc.stderr.resume();
 proc.on('close',()=>{
  runtimeFailure=Error('worker_closed');
  for(const p of pending.values()){clearTimeout(p.timer);p.reject(runtimeFailure);}
  pending.clear();
 });
 const send=m=>proc.stdin.write(JSON.stringify(m)+'\n');
 const lines=readline.createInterface({input:proc.stdout});
 lines.on('line',line=>{
  let msg;try{msg=JSON.parse(line);}catch{return;}
  try{
   if(!msg.method&&Object.hasOwn(msg,'id')&&pending.has(msg.id)){
    const p=pending.get(msg.id);pending.delete(msg.id);clearTimeout(p.timer);
    msg.error?p.reject(Error('acp_error')):p.resolve(msg.result);return;
   }
   if(msg.method==='session/update'){update(msg.params?.update);return;}
   if(Object.hasOwn(msg,'id')&&msg.method==='session/request_permission'){
    closeMessage();denied=true;emit('task.needs_action',{reason:'tool_permission_not_available'});
    send({jsonrpc:'2.0',id:msg.id,result:{outcome:{outcome:'cancelled'}}});return;
   }
   if(Object.hasOwn(msg,'id'))send({jsonrpc:'2.0',id:msg.id,error:{code:-32601,message:'unsupported'}});
  }catch(e){runtimeFailure=e;}
 });
 function rpc(method,params){
  return new Promise((resolve,reject)=>{
   const id=++counter;
   const timer=setTimeout(()=>{pending.delete(id);reject(Error('acp_timeout'));},155000);
   pending.set(id,{resolve,reject,timer});send({jsonrpc:'2.0',id,method,params});
  });
 }
 const init=await rpc('initialize',{protocolVersion:1,
   clientCapabilities:{fs:{readTextFile:false,writeTextFile:false},terminal:false},
   clientInfo:{name:'local-bridge-web-v2',version:'0.1'}});
 if(!init?.protocolVersion)throw Error('acp_init_failed');
 const session=await rpc('session/new',{cwd:work,mcpServers:[]});
 if(typeof session?.sessionId!=='string')throw Error('acp_session_failed');
 const response=await rpc('session/prompt',{sessionId:session.sessionId,
   prompt:[{type:'text',text:prompt}]});
 closeMessage();
 if(runtimeFailure||denied||response?.stopReason!=='end_turn'||!messages)
  throw Error(denied?'permission_not_available':'acp_not_completed');
 emit('task.completed',{stop_reason:'end_turn'});
}
try{
 await Promise.race([task(),new Promise((_,reject)=>setTimeout(()=>reject(Error('task_timeout')),170000))]);
}catch(e){
 try{closeMessage();emit('task.failed',{code:String(e.message||'unavailable').slice(0,64)});}catch{}
 process.exitCode=1;
}finally{
 for(const p of pending.values())clearTimeout(p.timer);pending.clear();
 try{proc?.kill('SIGTERM');}catch{}
 try{broker?.dispose();}catch{}
 try{server?.closeAllConnections();server?.close();}catch{}
}
