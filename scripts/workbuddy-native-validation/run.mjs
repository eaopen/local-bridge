// Experimental macOS-only validation; not a general AgentAdapter.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import http from 'node:http';
import {spawn} from 'node:child_process';
import {randomBytes,timingSafeEqual} from 'node:crypto';
import readline from 'node:readline';
import {connectWorkbuddyBroker} from './workbuddyBroker.ts';
import {loadWorkbuddyModelConfig} from './workbuddyModels.ts';
const root=fs.mkdtempSync('/tmp/local-bridge-wb-native-'); fs.chmodSync(root,0o700);
const work=path.join(root,'workspace');fs.mkdirSync(work,{mode:0o700});
const config=path.join(root,'config');fs.mkdirSync(config,{mode:0o700});
const cli='/Applications/WorkBuddy.app/Contents/Resources/app.asar.unpacked/cli';
const nonce=randomBytes(32).toString('hex');
const receipt={root,transport:'WBIPC authenticated fetch + bundled CLI ACP',events:[],requests:[]};
function log(event,data={}){receipt.events.push({event,...data});console.log(JSON.stringify({event,...data}));}
let broker,server,proc;const pending=new Map();let id=0;let assistant='';
try {
 broker=await connectWorkbuddyBroker(path.join(os.homedir(),'.workbuddy'));
 log('broker_connected');
 const modelConfig=await loadWorkbuddyModelConfig(broker,JSON.parse(fs.readFileSync(path.join(cli,'product.json'),'utf8')));
 log('catalog_loaded',{models:modelConfig.agents.find(x=>x.name==='cli').models});
 server=http.createServer(async(req,res)=>{try{
 const a=Buffer.from(req.headers.authorization||''),b=Buffer.from('Bearer '+nonce);
 if(req.headers.origin||a.length!==b.length||!timingSafeEqual(a,b)){res.writeHead(401);return res.end();}
 const u=new URL(req.url,'http://127.0.0.1');if(!/^\/[A-Za-z0-9_./-]*$/.test(u.pathname)||req.url.startsWith('//'))throw Error('path');
 let chunks=[],size=0;for await(const chunk of req){size+=chunk.length;if(size>655360)throw Error('size');chunks.push(chunk);}
 const headers={};for(const k of ['content-type','accept'])if(typeof req.headers[k]==='string')headers[k]=req.headers[k];
 const result=await broker.requestFetch({method:req.method,path:u.pathname,query:Object.fromEntries(u.searchParams),headers,...(size?{body_b64:Buffer.concat(chunks).toString('base64')}:{})});
 receipt.requests.push({method:req.method,path:u.pathname,status:result.status});
 log('proxy_response',{path:u.pathname,status:result.status});
 if(result.status>=300&&result.status<400)throw Error('redirect');
 const body=Buffer.from(result.body_b64,'base64');if(body.length>655360)throw Error('size');
 res.writeHead(result.status,{'content-type':result.headers['content-type']||'application/json'});res.end(body);
 }catch(e){log('proxy_failed',{code:e.code||'transport_error'});res.writeHead(422);res.end('{"error":{"message":"local transport failed"}}');}});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));const origin='http://127.0.0.1:'+server.address().port;
 const env={...process.env};for(const k of Object.keys(env))if(/^(CODEBUDDY_|WORKBUDDY_|ACC_PRODUCT_CONFIG|ACC_USER_|ANTHROPIC_|OPENAI_)/.test(k))delete env[k];
 Object.assign(env,{ELECTRON_RUN_AS_NODE:'1',CODEBUDDY_FORCE_HEADLESS_BUNDLE:'1',CODEBUDDY_CONFIG_DIR:config,WORKBUDDY_CONFIG_DIR:config,CODEBUDDY_AUTH_TOKEN:nonce,CODEBUDDY_BASE_URL:origin+'/v2',CODEBUDDY_INTERNET_ENVIRONMENT:'external',CODEBUDDY_CREDENTIALS_IN_MEMORY:'1',CODEBUDDY_DISABLE_LOCAL_STORAGE:'1',CODEBUDDY_DISABLE_PRODUCT_CACHE:'1',CODEBUDDY_DISABLE_CUSTOM_MODELS_FILE:'1',CODEBUDDY_API_KEY_DISABLED:'1',CODEBUDDY_API_KEY_HELPER_DISABLED:'1',CODEBUDDY_MAX_RETRIES:'0',CODEBUDDY_RETRY_WATCHDOG:'0',CODEBUDDY_GATEWAY_AUTH:'password',CODEBUDDY_GATEWAY_PASSWORD:randomBytes(32).toString('hex'),CODEBUDDY_GATEWAY_DISABLE_API_DOCS:'1',DISABLE_AUTOUPDATER:'1',DISABLE_TELEMETRY:'1',ACC_PRODUCT_CONFIG_V3:JSON.stringify({...modelConfig,endpoint:origin,networkEnvironment:'external',authentication:{type:'custom-token',attributes:{tokenType:'bearerToken',token:nonce}}})});
 proc=spawn('/Applications/WorkBuddy.app/Contents/MacOS/Electron',[path.join(cli,'dist/codebuddy-lite-wb.mjs'),'--serve','--port','0','--host','127.0.0.1','--acp','--acp-transport','stdio','--setting-sources','none','--strict-mcp-config','--mcp-config','{"mcpServers":{}}','--tools','Read,Write','--no-session-persistence'],{cwd:work,env,stdio:['pipe','pipe','pipe']});
 receipt.worker_pid=proc.pid;log('worker_started',{pid:proc.pid,root});
 proc.stderr.on('data',()=>{});proc.on('close',(code)=>{log('worker_exit',{code});for(const p of pending.values()){clearTimeout(p.timer);p.reject(Error('worker exited before response'));}pending.clear();});
 const send=x=>proc.stdin.write(JSON.stringify(x)+'\n');
 const lines=readline.createInterface({input:proc.stdout});
 lines.on('line',line=>{let m;try{m=JSON.parse(line);}catch{return;}
 if(!m.method && Object.hasOwn(m,"id") && pending.has(m.id)){const p=pending.get(m.id);pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error('ACP error '+JSON.stringify(m.error))):p.resolve(m.result);return;}
 if(m.method==='session/update'){const u=m.params?.update;if(u?.sessionUpdate==='agent_message_chunk'&&u.content?.type==='text')assistant+=u.content.text;if(['tool_call','tool_call_update'].includes(u?.sessionUpdate))log('tool_event',{kind:u.sessionUpdate,title:u.title,status:u.status,toolCallId:u.toolCallId});}
 else if(Object.hasOwn(m,'id') && m.method==='session/request_permission'){const t=m.params.toolCall;log('permission_request',{toolCall:t,options:m.params.options});const raw=t.rawInput||{};const target=path.join(work,'bridge-workbuddy-test.txt');const locs=t.locations||[];const scoped=t._meta?.['codebuddy.ai/toolName']==='Write' && raw.file_path===target && raw.content==='LOCAL_BRIDGE_WORKBUDDY_OK' && !fs.existsSync(target);const option=m.params.options?.find(x=>x.kind==='allow_once');if(scoped&&option){log('permission_allowed',{path:target,kind:'allow_once'});send({jsonrpc:'2.0',id:m.id,result:{outcome:{outcome:'selected',optionId:option.optionId}}});}else{log('permission_denied');send({jsonrpc:'2.0',id:m.id,result:{outcome:{outcome:'cancelled'}}});}}
 else if(Object.hasOwn(m,'id')){log('client_request',{method:m.method,params:m.params});send({jsonrpc:'2.0',id:m.id,error:{code:-32601,message:'Not supported in first validation'}});}
 });
 function rpc(method,params){return new Promise((resolve,reject)=>{const n=++id;const timer=setTimeout(()=>{pending.delete(n);reject(Error(method+' timeout'));},150000);pending.set(n,{resolve,reject,timer});send({jsonrpc:'2.0',id:n,method,params});});}
 const init=await rpc('initialize',{protocolVersion:1,clientCapabilities:{fs:{readTextFile:false,writeTextFile:false},terminal:false},clientInfo:{name:'local-bridge-validation',version:'0.1'}});log('initialized',{protocolVersion:init.protocolVersion});
 const sess=await rpc('session/new',{cwd:work,mcpServers:[]});receipt.session_id=sess.sessionId;log('session_created',{sessionId:sess.sessionId});
 const result=await rpc('session/prompt',{sessionId:sess.sessionId,prompt:[{type:'text',text:'Use the Write file tool to create '+path.join(work,'bridge-workbuddy-test.txt')+' with exact content LOCAL_BRIDGE_WORKBUDDY_OK (no newline). Do not read or modify any other files, do not use shell or network tools. Actually write the file, then return its absolute path. This one test-file write is authorized.'}]});
 log('prompt_completed',{stopReason:result.stopReason,text:assistant});
 const target=path.join(work,'bridge-workbuddy-test.txt');receipt.file_verified=result.stopReason==='end_turn'&&fs.existsSync(target)&&fs.readFileSync(target,'utf8')==='LOCAL_BRIDGE_WORKBUDDY_OK';receipt.result_path=target;log('file_verification',{passed:receipt.file_verified,path:target});if(!receipt.file_verified)process.exitCode=1;
} catch(e){log('failed',{message:e.message});process.exitCode=1;}
finally{for(const p of pending.values())clearTimeout(p.timer);pending.clear();if(proc){proc.kill('SIGTERM');await new Promise(r=>{if(proc.exitCode!==null)return r();proc.once('close',r);setTimeout(()=>{proc.kill('SIGKILL');r();},5000).unref();});}broker?.dispose();server?.closeAllConnections();if(server)await new Promise(r=>server.close(r));fs.writeFileSync(path.join(root,'receipt.json'),JSON.stringify(receipt,null,2));if(process.env.LOCAL_BRIDGE_VALIDATION_RECEIPT)fs.writeFileSync(process.env.LOCAL_BRIDGE_VALIDATION_RECEIPT,JSON.stringify(receipt,null,2),{flag:'wx',mode:0o600});console.log(JSON.stringify({receipt:path.join(root,'receipt.json')}));}
