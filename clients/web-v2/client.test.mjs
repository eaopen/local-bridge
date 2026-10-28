import test from 'node:test'
import assert from 'node:assert/strict'
import {
  CLIENT_VERSION, createLocalBridgeClient, directJson, directStream
} from './client.js'

const ENDPOINT = 'http://127.0.0.1:48293'
const TOKEN = 'a'.repeat(64)
const SESSION = {endpoint: ENDPOINT, token: TOKEN, expires_in: 600}
const TASK = 'wb-abcde-1234-0'
const ORIGIN = 'https://enterprise.example.test'

function browser(handler) {
  const listeners = new Set()
  const calls = []
  const page = {
    location: {origin: ORIGIN},
    addEventListener(type, fn) {assert.equal(type, 'message'); listeners.add(fn)},
    removeEventListener(type, fn) {assert.equal(type, 'message'); listeners.delete(fn)},
    postMessage(req) {
      assert.equal(req.channel, 'local-tool-bridge:web:v2')
      assert.equal(req.type, 'REQUEST')
      assert.equal(req.version, 2)
      assert.equal(req.params?.origin, undefined) // Chrome provides origin
      calls.push(req)
      Promise.resolve().then(() => {
        const res = handler(req)
        const answer = {channel: req.channel, type:'RESPONSE', version:2,
          requestId:req.requestId, ...res}
        for(const fn of listeners) fn({source:page,origin:ORIGIN,data:answer})
      })
    }
  }
  return {page, calls}
}
function capabilities() {
  return {protocol:'local-bridge-http-sse-v2',
    workbuddy:{installed:true, local_grant:true, run_supported:true}}
}
function jsonResponse(data,status=200) {
  return {ok:status>=200&&status<300,status,
    async json(){return data}}
}
function streamReply(frames) {
  const encoded=new TextEncoder().encode(frames.join(''))
  return {ok:true,status:200,body:new ReadableStream({
    start(c){
      c.enqueue(encoded.slice(0,17))
      c.enqueue(encoded.slice(17,113))
      c.enqueue(encoded.slice(113))
      c.close()
    }
  })}
}

test('SDK is a build-time, versioned, zero-dependency ESM browser client',()=>{
  assert.equal(CLIENT_VERSION,'2.0.0-preview.1')
})
test('one Chrome bootstrap then direct HTTP capabilities and MCP data',async()=>{
  const {page,calls}=browser(req=>({
    ok:true,
    result:req.action==='local.connect'?SESSION:{unavailable:true}
  }))
  const routes=[]
  const client=createLocalBridgeClient({page,fetchImpl:async(url,opts)=>{
    routes.push({url,opts})
    if(url===ENDPOINT+'/v2/capabilities')return jsonResponse(capabilities())
    if(url===ENDPOINT+'/v2/mcp/status')return jsonResponse({provider:'wechat',ok:true})
    throw Error('unknown route')
  }})
  const connection=await client.connect()
  assert.equal(connection.available,true)
  assert.equal(connection.mode,'direct')
  assert.equal(calls.length,1)
  assert.equal(calls[0].action,'local.connect')
  const status=await client.mcpStatus('wechat','wechat_status')
  assert.equal(status.ok,true)
  assert.equal(calls.length,1) // Chrome is not in the data path
  assert.equal(routes[1].opts.headers.Authorization,'Bearer '+TOKEN)
  assert.equal(routes[1].opts.credentials,'omit')
  assert.ok(!routes[1].url.includes(TOKEN))
})
test('submit WorkBuddy directly; network failure falls back with identical key',async()=>{
  const {page,calls}=browser(req=>({
    ok:true,result:req.action==='local.connect'?SESSION:
      req.action==='agent.start'?
        {task_id:TASK,status:'accepted',client_key:req.params.client_key}:capabilities()
  }))
  let failStart=false
  const client=createLocalBridgeClient({page,fetchImpl:async(url,opts)=>{
    if(url.endsWith('/v2/capabilities'))return jsonResponse(capabilities())
    if(url.endsWith('/v2/workbuddy/tasks')){
      if(failStart)throw new TypeError('Connection reset after accept')
      assert.equal(JSON.parse(opts.body).client_key,'key_123')
      return jsonResponse({task_id:TASK,status:'accepted'},202)
    }
    throw Error(url)
  }})
  await client.connect()
  assert.equal((await client.startWorkbuddy({clientKey:'key_123',prompt:'Synthetic task'})).status,'accepted')
  assert.equal(calls.filter(x=>x.action==='agent.start').length,0)
  failStart=true
  const second=await client.startWorkbuddy({clientKey:'key_123',prompt:'Synthetic task'})
  assert.equal(second.client_key,'key_123')
  assert.equal(calls.filter(x=>x.action==='agent.start').length,1)
  assert.equal(client.mode,'native-fallback')
})
test('no plugin or Host is feature unavailable rather than a broken enterprise page',async()=>{
  const {page}=browser(()=>({ok:false,code:'no_extension'}))
  const client=createLocalBridgeClient({page,fetchImpl:async()=>{throw Error('should not connect')}})
  const connection=await client.connect()
  assert.deepEqual({available:connection.available,mode:connection.mode},
    {available:false,mode:'unavailable'})
})
test('multi-message SSE retains per-message events and separate terminal event',async()=>{
  const raw=[
    'id: 1\nevent: message.delta\ndata: {"seq":1,"type":"message.delta","message_id":"m1","text":"Hello"}\n\n',
    'id: 2\nevent: message.final\ndata: {"seq":2,"type":"message.final","message_id":"m1"}\n\n',
    'id: 3\nevent: message.delta\ndata: {"seq":3,"type":"message.delta","message_id":"m2","text":"Again"}\n\n',
    'id: 4\nevent: message.final\ndata: {"seq":4,"type":"message.final","message_id":"m2"}\n\n',
    'id: 5\nevent: task.completed\ndata: {"seq":5,"type":"task.completed"}\n\n',
    'event: task.status\ndata: {"type":"task.status","status":"completed","terminal":true}\n\n'
  ]
  const events=[]
  const seq=await directStream(SESSION,TASK,0,e=>events.push(e),
    undefined, async(url,opts)=>{
      assert.equal(url,ENDPOINT+'/v2/workbuddy/tasks/'+TASK+'/stream?cursor=0')
      assert.equal(opts.headers.Authorization,'Bearer '+TOKEN)
      return streamReply(raw)
    })
  assert.equal(seq,5)
  assert.deepEqual(events.map(e=>e.type),[
    'message.delta','message.final','message.delta','message.final','task.completed','task.status'])
  assert.equal(events[5].terminal,true)
})
test('stream disconnect after one message, native poll resumes at last seq without replay',async()=>{
  const first='id: 1\nevent: message.delta\ndata: {"seq":1,"type":"message.delta","message_id":"m1","text":"part"}\n\n'
  const {page,calls}=browser(req=>({
    ok:true,result:req.action==='local.connect'?SESSION:
      req.action==='agent.poll'?{
        task_id:TASK,status:'completed',terminal:true,next_seq:2,
        has_more:false,events:[{seq:2,type:'task.completed'}]
      }:capabilities()
  }))
  const client=createLocalBridgeClient({page,fetchImpl:async(url)=>{
    if(url.endsWith('/v2/capabilities'))return jsonResponse(capabilities())
    if(url.includes('/stream?cursor=0')){
      let step=0
      return {ok:true,status:200,body:new ReadableStream({
        pull(c) {
          if(step++===0) c.enqueue(new TextEncoder().encode(first))
          else c.error(new Error('broken_stream'))
        }
      })}
    }
    throw Error(url)
  }})
  await client.connect()
  const received=[]
  const res=await client.readWorkbuddy(TASK,0,e=>received.push(e))
  assert.deepEqual(received.map(e=>e.seq),[1,2])
  assert.equal(calls.find(c=>c.action==='agent.poll').params.cursor,1)
  assert.equal(res.status,'completed')
  assert.equal(res.transport,'native-fallback')
})
test('fixed loopback, capability and task validation block arbitrary URLs and tools',async()=>{
  await assert.rejects(()=>directJson({...SESSION,endpoint:'http://evil.example.test:48293'},
    '/v2/capabilities',{},async()=>jsonResponse({})),/invalid_local_session/)
  const {page}=browser(req=>({ok:true,result:req.action==='local.connect'?SESSION:capabilities()}))
  const client=createLocalBridgeClient({page,fetchImpl:async()=>jsonResponse(capabilities())})
  await client.connect()
  await assert.rejects(()=>client.mcpStatus('wechat','wechat_send_message'),/invalid_mcp_status/)
  await assert.rejects(()=>client.startWorkbuddy({clientKey:'../../',prompt:'hi'}),/invalid_client_key/)
  await assert.rejects(()=>client.pollWorkbuddy('../unscoped',0),/invalid_task_id/)
})
