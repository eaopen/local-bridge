import test from 'node:test'
import assert from 'node:assert/strict'
import {agentHealthView,createAgentCheckQueue} from './agent-health.mjs'
const installed={installed:true,run_supported:true}
const completed={ready:true,authenticated:true,api_ready:true,run_completed:true,checked_at_unix_ms:1000}
test('installed, authenticated or accepted alone never means usable',()=>{
  assert.equal(agentHealthView(installed,null,false,1000).label,'待检查')
  for(const incomplete of [{ready:true},{...completed,run_completed:false},{...completed,authenticated:null}])
    assert.notEqual(agentHealthView(installed,incomplete,false,1000).label,'可用')
  assert.equal(agentHealthView(installed,completed,false,1000).label,'可用')
})
test('expired or missing installations override an earlier success',()=>{
  assert.equal(agentHealthView(installed,completed,false,302000).label,'需重新检查')
  assert.equal(agentHealthView({installed:false},completed,false,1000).label,'未安装')
  assert.equal(agentHealthView(installed,completed,true,1000).label,'检查中')
})

test('startup checks serialize, coalesce clicks, continue after failure and allow retry',async()=>{
  const started=[],snapshots=[]
  let release,active=0,maximum=0
  const first=new Promise(resolve=>{release=resolve})
  const queue=createAgentCheckQueue(async agent=>{
    started.push(agent);active++;maximum=Math.max(maximum,active)
    try{
      if(agent==='workbuddy')await first
      if(agent==='pi')throw new Error('unavailable')
      return agent
    }finally{active--}
  },snapshot=>snapshots.push(snapshot))
  const workbuddy=queue.enqueue('workbuddy')
  assert.equal(queue.enqueue('workbuddy'),workbuddy)
  const codex=queue.enqueue('codex'),pi=queue.enqueue('pi')
  assert.deepEqual(started,['workbuddy'])
  assert.deepEqual(snapshots.at(-1),{active:'workbuddy',pending:['codex','pi']})
  release()
  const outcomes=await Promise.allSettled([workbuddy,codex,pi])
  assert.deepEqual(outcomes.map(x=>x.status),['fulfilled','fulfilled','rejected'])
  await assert.rejects(queue.enqueue('pi'),/unavailable/)
  assert.deepEqual(started,['workbuddy','codex','pi','pi'])
  assert.equal(maximum,1)
  assert.deepEqual(snapshots.at(-1),{active:null,pending:[]})
})
