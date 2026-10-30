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
