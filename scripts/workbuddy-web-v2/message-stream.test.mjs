import test from 'node:test';
import assert from 'node:assert/strict';
import {createMessageStream} from './message-stream.mjs';
const chunk=text=>({sessionUpdate:'agent_message_chunk',content:{type:'text',text}});
const snapshot=text=>({sessionUpdate:'agent_message',content:{type:'text',text}});
function setup(){const events=[];return {events,stream:createMessageStream((type,body)=>{events.push({type,...body});if(events.length>250)throw Error('event_limit')})};}
test('long token stream fits event budget and preserves exact Unicode text',()=>{
 const {events,stream}=setup();const text='虚构回复🙂'.repeat(500);
 for(const token of text)stream.update(chunk(token));stream.close();
 assert.equal(events.filter(e=>e.type==='message.delta').map(e=>e.text).join(''),text);
 assert.equal(stream.count,1);assert.ok(events.length<10);assert.equal(events.at(-1).type,'message.final');
});
test('matching final snapshot does not duplicate buffered deltas',()=>{
 const {events,stream}=setup();stream.update(chunk('draft'));stream.update(snapshot('draft'));
 assert.deepEqual(events,[{type:'message.delta',message_id:'msg-1',text:'draft'},{type:'message.final',message_id:'msg-1'}]);
});
test('replacement snapshots and tool boundaries flush separate messages',()=>{
 const {events,stream}=setup();stream.update(chunk('old'));stream.update(snapshot('replacement'));
 stream.update(chunk('new'));stream.update({sessionUpdate:'tool_call',status:'pending'});
 assert.deepEqual(events.filter(e=>e.type==='message.delta').map(e=>[e.message_id,e.text]),[['msg-1','old'],['msg-2','replacement'],['msg-3','new']]);
 assert.equal(events.at(-1).type,'tool.update');
});

test('chunk boundaries never serialize unpaired Unicode surrogates',()=>{
 const {events,stream}=setup();const text='x'.repeat(1023)+'🙂'+'y'.repeat(1024);
 stream.update(chunk(text));stream.close();
 for(const event of events.filter(e=>e.type==='message.delta'))assert.equal(Buffer.from(event.text).toString('utf8'),event.text);
 assert.equal(events.filter(e=>e.type==='message.delta').map(e=>e.text).join(''),text);
});
