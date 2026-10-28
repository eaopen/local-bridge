import test from 'node:test'
import assert from 'node:assert/strict'
import {AGENTS, defaults, normalizePreferenceDraft, shiftPriority, THINK_OPTIONS} from './preferences.mjs'

test('default settings are complete, with unique real agents',()=>{
  const p=normalizePreferenceDraft(defaults())
  assert.equal(p.schema_version,1)
  assert.deepEqual(p.priority,AGENTS)
  assert.equal(p.agent,'auto')
  assert.equal(p.think_mode,'auto')
})

test('priority can be reordered without creating or deleting agents',()=>{
  const changed=shiftPriority(AGENTS,'pi',-1)
  assert.deepEqual(changed,['workbuddy','pi','codex'])
  assert.deepEqual(AGENTS,['workbuddy','codex','pi'])
  assert.deepEqual(shiftPriority(changed,'pi',-1),['pi','workbuddy','codex'])
})

test('desktop validation never stores provider credentials or impossible settings',()=>{
  assert.throws(()=>normalizePreferenceDraft({...defaults(),agent:'workbuddy',model:'gpt-test'}))
  assert.throws(()=>normalizePreferenceDraft({...defaults(),agent:'auto',agent_mode:'require'}))
  assert.throws(()=>normalizePreferenceDraft({...defaults(),priority:['codex','codex','pi']}))
  assert.throws(()=>normalizePreferenceDraft({...defaults(),model:'https://bad.example/token'}))
  assert.throws(()=>normalizePreferenceDraft({...defaults(),agent:'codex',think_mode:'off'}))
  assert.equal(normalizePreferenceDraft({...defaults(),agent:'pi',think_mode:'off'}).think_mode,'off')
  assert.equal(THINK_OPTIONS.workbuddy.length,1)
})
