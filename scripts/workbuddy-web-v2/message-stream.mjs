// Coalesce token-sized ACP updates without losing message/snapshot boundaries.
// The worker's event/byte limits still bound the entire run.
export function createMessageStream(emit){
 let current='',pending='',id=null,count=0;
 function flush(){if(pending){emit('message.delta',{message_id:id,text:pending});pending='';}}
 function append(text){
  if(!id)id='msg-'+(++count);
  current+=text;pending+=text;
  while(pending.length>=1024){
   const end=/[\uD800-\uDBFF]/.test(pending[1023])?1023:1024;
   emit('message.delta',{message_id:id,text:pending.slice(0,end)});
   pending=pending.slice(end);
  }
 }
 function close(){if(id){flush();emit('message.final',{message_id:id});id=null;current='';}}
 function update(u){
  if(!u||typeof u!=='object')return;
  if(u.sessionUpdate==='agent_message_chunk'&&u.content?.type==='text'){
   if(typeof u.content.text==='string'&&u.content.text)append(u.content.text);
  }else if(u.sessionUpdate==='agent_message'&&u.content?.type==='text'){
   const full=String(u.content.text||'');
   if(full&&full!==current){close();append(full);}
   close();
  }else if(['tool_call','tool_call_update'].includes(u.sessionUpdate)){
   close();emit('tool.update',{state:String(u.status||'unknown').slice(0,24)});
  }
 }
 return {update,close,get count(){return count}};
}
