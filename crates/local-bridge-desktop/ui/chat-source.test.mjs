import test from 'node:test'
import assert from 'node:assert/strict'
import {selectedContext} from './chat-source.mjs'
const data={source:'local_index',chat_id:'虚构会话',messages:[{text:'测试一',sender:'甲'},{text:'测试二'}]}
test('only selected messages enter AI input and retain non-live source',()=>{
 assert.deepEqual(selectedContext(data,[1]),{text:'发言人未知：测试二',context_source:'wechat_local_index',thread_label:'虚构会话'})
 assert.throws(()=>selectedContext(data,[]))
 assert.throws(()=>selectedContext(data,[9]))
 assert.throws(()=>selectedContext({...data,source:'unknown'},[0]))
 assert.throws(()=>selectedContext({...data,messages:[{text:'中'.repeat(3000)}]},[0]))
})
test('structured DB source must be explicit and preserves stable source',()=>{
 const structured={source:'structured_db',chat_id:'wxid_synthetic',id_kind:'stable_conversation_id',source_identity:'sha256:synthetic',messages:[{text:'测试片段',sender:'甲'}]}
 assert.deepEqual(selectedContext(structured,[0]),{text:'甲：测试片段',context_source:'wechat_structured_db',thread_label:'wxid_synthetic'})
 assert.equal(selectedContext(structured,[0],'团队讨论群').thread_label,'团队讨论群')
 assert.equal(selectedContext(structured,[0],''.padEnd(201,'a')).thread_label,'wxid_synthetic')
})

test('picker defaults to configured records and uses stable ID for same-name chats',async()=>{
 const {mountChatSource}=await import('./chat-source.mjs')
 class Element{
  value='';children=[];events={};textContent=''
  addEventListener(name,fn){this.events[name]=fn}
  append(...children){this.children.push(...children)}
  replaceChildren(){this.children=[]}
  querySelector(){return new Element()}
 }
 const elements=new Map(),document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id)},createElement(){return new Element()}}
 const el=id=>document.getElementById(id),calls=[]
 el('chat-source-mode').value='local_index'
 mountChatSource({document,onBusy(){},onUse(){},async call(command,args){
  calls.push([command,args])
  if(command==='desktop_chat_source_status')return {read_contract_supported:true,structured_db_read:true}
  if(command==='desktop_chat_source_conversations')return args.cursor?{chats:[{chat_id:'stable-c',title:'后续页目标群'}],has_more:false}:{chats:[{chat_id:'stable-a',title:'同名'},{chat_id:'stable-b',title:'同名'}],has_more:true,next_cursor:'page2'}
  return {source:'structured_db',chat_id:args.chatId,messages:[]}
 }})
 el('chat-source-check').events.click();await new Promise(resolve=>setImmediate(resolve))
 assert.equal(el('chat-source-mode').value,'structured_db')
 assert.equal(el('chat-source-conversation').children.length,3)
 assert.notEqual(el('chat-source-conversation').children[1].textContent,el('chat-source-conversation').children[2].textContent)
 el('chat-source-conversation').value='stable-b';el('chat-source-conversation').events.change()
 el('chat-source-read').events.click();await new Promise(resolve=>setImmediate(resolve))
 assert.equal(calls.at(-1)[1].chatId,'stable-b')
 el('chat-source-search').value='目标群';el('chat-source-search').events.input()
 await new Promise(resolve=>setTimeout(resolve,450))
 assert.equal(el('chat-source-id').value,'stable-c')
 assert.equal(el('chat-source-read').disabled,false)
 assert.equal(calls.filter(([name])=>name==='desktop_chat_source_read').length,1)
 assert.equal(calls.some(([name])=>name.includes('send')||name.includes('run')),false)
})

test('a second provider keeps its own source and rejects cross-provider use',()=>{
 const web={source:'web_page',provider:'soul',chat_id:'网页会话',live_read:true,session_visible_only:true,messages:[{text:'网页片段',sender:'对方'}]}
 assert.deepEqual(selectedContext(web,[0],undefined,'soul'),{text:'对方：网页片段',context_source:'soul_web_page',thread_label:'网页会话'})
 assert.deepEqual(selectedContext(web,[0],'可读名称','soul'),{text:'对方：网页片段',context_source:'soul_web_page',thread_label:'可读名称'})
 assert.throws(()=>selectedContext(web,[0],undefined,'wechat'))
 assert.throws(()=>selectedContext(data,[0],undefined,'soul'))
 assert.throws(()=>selectedContext(data,[0],undefined,'unknown'))
 assert.throws(()=>selectedContext({...web,provider:'wechat'},[0],undefined,'soul'))
 assert.throws(()=>selectedContext({...web,live_read:false},[0],undefined,'soul'))
 assert.throws(()=>selectedContext({...web,session_visible_only:false},[0],undefined,'soul'))
})

test('web page provider reads a named visible session without listing or sending',async()=>{
 const {mountChatSource}=await import('./chat-source.mjs')
 class Element{
  value='';children=[];events={};textContent='';disabled=false;hidden=false
  addEventListener(name,fn){this.events[name]=fn}
  append(...children){this.children.push(...children)}
  replaceChildren(){this.children=[]}
  querySelector(){return new Element()}
 }
 const elements=new Map(),document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id)},createElement(){return new Element()}}
 const el=id=>document.getElementById(id),calls=[]
 el('chat-source-provider').value='soul'
 mountChatSource({document,onBusy(){},onUse(){},async call(command,args){
  calls.push([command,args])
  if(command==='desktop_chat_source_status')return {registered:true,read_contract_supported:true,web_page_read:true}
  if(command==='desktop_chat_source_read')return {provider:'soul',source:'web_page',chat_id:args.chatId,id_kind:'display_name',live_read:true,session_visible_only:true,messages:[{text:'网页片段',sender:'对方',source:'cache'}]}
  return {}
 }})
 el('chat-source-check').events.click();await new Promise(resolve=>setImmediate(resolve))
 assert.equal(calls[0][1].provider,'soul')
 assert.equal(el('chat-source-mode').value,'web_page')
 assert.equal(calls.some(([name])=>name==='desktop_chat_source_conversations'),false)
 el('chat-source-id').value='网页会话';el('chat-source-id').events.input()
 assert.equal(el('chat-source-read').disabled,false)
 el('chat-source-read').events.click();await new Promise(resolve=>setImmediate(resolve))
 const read=calls.find(([name])=>name==='desktop_chat_source_read')
 assert.deepEqual(read[1],{provider:'soul',chatId:'网页会话',source:'web_page',limit:10})
 assert.equal(calls.some(([name])=>name.includes('send')||name.includes('run')),false)
})

test('a provider without the declared capability cannot be read',async()=>{
 const {mountChatSource}=await import('./chat-source.mjs')
 class Element{
  value='';children=[];events={};textContent='';disabled=false;hidden=false
  addEventListener(name,fn){this.events[name]=fn}
  append(...children){this.children.push(...children)}
  replaceChildren(){this.children=[]}
  querySelector(){return new Element()}
 }
 const elements=new Map(),document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id)},createElement(){return new Element()}}
 const el=id=>document.getElementById(id),calls=[]
 el('chat-source-provider').value='soul'
 mountChatSource({document,onBusy(){},onUse(){},async call(command,args){
  calls.push([command,args])
  if(command==='desktop_chat_source_status')return {registered:true,read_contract_supported:true}
  return {}
 }})
 el('chat-source-check').events.click();await new Promise(resolve=>setImmediate(resolve))
 assert.equal(el('chat-source-read').disabled,true)
 el('chat-source-id').value='网页会话';el('chat-source-id').events.input()
 el('chat-source-read').events.click();await new Promise(resolve=>setImmediate(resolve))
 assert.equal(calls.some(([name])=>name==='desktop_chat_source_read'),false)
 assert.equal(el('chat-source-list-fields').hidden,true)
})

test('wechat prefers the stable structured DB when several sources are usable',async()=>{
 const {mountChatSource}=await import('./chat-source.mjs')
 class Element{
  value='';children=[];events={};textContent='';disabled=false;hidden=false
  addEventListener(name,fn){this.events[name]=fn}
  append(...children){this.children.push(...children)}
  replaceChildren(){this.children=[]}
  querySelector(){return new Element()}
 }
 const elements=new Map(),document={getElementById(id){if(!elements.has(id))elements.set(id,new Element());return elements.get(id)},createElement(){return new Element()}}
 const el=id=>document.getElementById(id)
 mountChatSource({document,onBusy(){},onUse(){},async call(command){
  if(command==='desktop_chat_source_status')return {read_contract_supported:true,local_index_read:true,structured_db_read:true}
  if(command==='desktop_chat_source_conversations')return {chats:[],has_more:false}
  return {}
 }})
 el('chat-source-check').events.click();await new Promise(resolve=>setImmediate(resolve))
 assert.equal(el('chat-source-mode').value,'structured_db')
})
