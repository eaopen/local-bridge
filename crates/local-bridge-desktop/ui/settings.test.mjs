import test from 'node:test'
import assert from 'node:assert/strict'
import {SETTINGS,generalPreferences,connectionState} from './settings.mjs'

test('invalid saved UI settings recover to safe defaults without accepting navigation URLs',()=>{
  assert.deepEqual(generalPreferences(null),{theme:'system',startPage:'overview'})
  assert.deepEqual(generalPreferences({theme:'invalid',startPage:'https://example.test'}),
    {theme:'system',startPage:'overview'})
  assert.deepEqual(generalPreferences({theme:'dark',startPage:'settings'}),
    {theme:'dark',startPage:'settings'})
})
test('ecosystem is an optional settings view',()=>{
  assert.ok(SETTINGS.includes('ecosystem'))
  assert.ok(SETTINGS.includes('agent'))
})
test('registration, MCP response and app login remain distinct; removal discards stale success',()=>{
  assert.equal(connectionState(false,{tools:['wechat_status']}).label,'未载入')
  assert.equal(connectionState(true,null).label,'已载入 · 待检查')
  assert.equal(connectionState(true,{error:'timeout'}).kind,'bad')
  const connected=connectionState(true,{tools:['wechat_status']})
  assert.equal(connected.label,'已连接')
  assert.match(connected.detail,/账号登录.*Provider 核验/)
})
