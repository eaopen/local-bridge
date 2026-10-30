import test from 'node:test';
import assert from 'node:assert/strict';
import {modelOptions} from './model-options.mjs';
const catalog={agents:[{name:'cli',models:['auto','flash','fixed']}],models:[
 {id:'auto',reasoning:{effort:'high'}},
 {id:'flash',reasoning:{supportedEfforts:['low','high','max']}},
 {id:'fixed',reasoning:{effort:'medium'}},{id:'unlisted'}
]};
test('default keeps vendor model and reasoning selection',()=>assert.deepEqual(modelOptions(catalog),[]));
test('explicit supported selection reaches CLI without fallback',()=>{
 assert.deepEqual(modelOptions(catalog,'flash','low'),['--model','flash','--effort','low']);
 assert.deepEqual(modelOptions(catalog,'fixed','auto'),['--model','fixed']);
});
test('missing, unlisted and malformed models fail before inference',()=>{
 for(const model of ['missing','unlisted','--model','https://example.com',null])
  assert.throws(()=>modelOptions(catalog,model),/model_unavailable/);
 assert.throws(()=>modelOptions({...catalog,models:[]},'flash'),/model_unavailable/);
});
test('unsupported or unknown reasoning capability is never silently ignored',()=>{
 for(const [model,effort] of [['flash','medium'],['auto','low'],['fixed','off'],['flash','--help']])
  assert.throws(()=>modelOptions(catalog,model,effort),/effort_unavailable/);
 assert.throws(()=>modelOptions({...catalog,models:[{id:'flash'}]},'flash','low'),/effort_unavailable/);
});
