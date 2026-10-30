import test from 'node:test'
import assert from 'node:assert/strict'
import {parseDraft,copyDraft,readUntilDone,FEEDBACK,relationshipHint,RELATIONSHIP_HINTS} from './chat-copilot.mjs'
const draft={judgment:'原因未知',facts:['两天未回复'],hypotheses:['可能忙，未经证实'],unknowns:['原因'],
  candidates:[{label:'稳',text:'先不下结论'},{label:'留边界',text:'这次不回复'}],avoid:'不推断感情'}
test('relationship labels guide context without automatic profiling',()=>{
  assert.equal(Object.keys(RELATIONSHIP_HINTS).length,7)
  assert.match(relationshipHint('work'),/避免擅自承诺/)
  assert.match(relationshipHint('new_connection'),/低压力/)
  assert.equal(relationshipHint('unknown'),relationshipHint('general'))
})
test('structured and fenced drafts keep facts and hypotheses separate',()=>{
  assert.deepEqual(parseDraft(JSON.stringify(draft)),draft)
  assert.deepEqual(parseDraft('```json\n'+JSON.stringify(draft)+'\n```'),draft)
  for(const raw of ['bad','null','{}',JSON.stringify({...draft,candidates:[]}),JSON.stringify({...draft,unknowns:'guess'}),'x'.repeat(32001)])
    assert.throws(()=>parseDraft(raw))
})
test('model text stays text, including markup',()=>{
  assert.equal(parseDraft(JSON.stringify({...draft,judgment:'<script>bad()</script>'})).judgment,'<script>bad()</script>')
})
test('copy uses the human edited draft and does not report failed writes as success',async()=>{
  let copied
  await copyDraft('我修改后的回复',{writeText:async value=>{copied=value}})
  assert.equal(copied,'我修改后的回复')
  await assert.rejects(copyDraft(' ',{writeText:async()=>{}}))
  await assert.rejects(copyDraft('草稿',null))
  await assert.rejects(copyDraft('草稿',{writeText:async()=>{throw Error('denied')}}))
  assert.deepEqual(FEEDBACK,['自然','太油','太冷','不像我','越界'])
})
test('polling reads the same invocation; failure and unknown never rerun an Agent',async()=>{
  const calls=[];let count=0
  const result=await readUntilDone(async(name,args)=>{
    calls.push([name,args]);return ++count===1?{status:'running'}:{status:'completed',result:'real receipt'}
  },'one',{delay:async()=>{}})
  assert.equal(result.result,'real receipt')
  assert.deepEqual(calls,[['desktop_chat_read',{id:'one'}],['desktop_chat_read',{id:'one'}]])
  for(const status of ['failed','unknown','submitted'])
    await assert.rejects(readUntilDone(async()=>({status}),'one',{delay:async()=>{}}))
  await assert.rejects(readUntilDone(async()=>({status:'running'}),'one',{delay:async()=>{},limit:2}),/等待超时/)
})

test('WorkBuddy fenced JSON with a trailing explanation renders without discarding the explanation',()=>{
  const fenced='```json\n'+JSON.stringify(draft)+'\n```'
  assert.deepEqual(parseDraft(fenced+'\n\n也可以等待，不必回复。'),{...draft,commentary:'也可以等待，不必回复。'})
  assert.throws(()=>parseDraft(fenced+'\n'+fenced))
  assert.throws(()=>parseDraft(fenced+'\n'+'x'.repeat(2001)))
  assert.throws(()=>parseDraft('```json\n{broken}\n```\n说明'))
})
