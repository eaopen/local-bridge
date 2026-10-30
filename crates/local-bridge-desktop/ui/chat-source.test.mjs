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
})
