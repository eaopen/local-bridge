// Explicit local-user reads only. No inference, sends, auto polling or persistence.
export function selectedContext(data,indexes,displayName){
 if(!['local_index','desktop_ui','structured_db'].includes(data?.source)||typeof data.chat_id!=='string'||!Array.isArray(data.messages))throw new Error('来源无效')
 const text=indexes.map(i=>{const m=data.messages[i];if(!m||typeof m.text!=='string')throw new Error('片段无效');return `${m.sender||'发言人未知'}：${m.text}`}).join('\n')
 if(!text.trim()||new TextEncoder().encode(text).length>7800)throw new Error('请选择少量片段，总长度需小于 7800 字节。')
 const label=typeof displayName==='string'&&displayName.trim()&&displayName.length<=200?
  displayName.trim():data.chat_id
 return {text,context_source:'wechat_'+data.source,thread_label:label}
}
export function mountChatSource({document,call,onBusy,onUse}){
 const el=id=>document.getElementById(id)
 let pending=false,blocked=false,data=null,chats=[],cursor=null,searchTimer=null,loaded=false
 const note=text=>{el('chat-source-status').textContent=text}
 function controls(){el('chat-source-read').disabled=pending||blocked||!el('chat-source-id').value.trim();el('chat-source-fields').disabled=pending||blocked;el('chat-source-apply').disabled=pending||blocked||!data}
 function reset(){data=null;el('chat-source-messages').replaceChildren();controls()}
 function renderChats(){
  const select=el('chat-source-conversation'),selected=el('chat-source-id').value
  const search=el('chat-source-search').value.trim().toLocaleLowerCase()
  select.replaceChildren()
  const placeholder=document.createElement('option');placeholder.value='';placeholder.textContent='请选择聊天';select.append(placeholder)
  for(const chat of chats.filter(c=>(c.title||c.chat_id).toLocaleLowerCase().includes(search))){
   const option=document.createElement('option');option.value=chat.chat_id
   const title=chat.title||chat.chat_id
   option.textContent=title+(chats.filter(c=>(c.title||c.chat_id)===title).length>1?' · '+chat.chat_id.slice(-12):'')
   select.append(option)
  }
  select.value=selected
  el('chat-source-more').hidden=!cursor
 }
 async function loadChats(more=false){
  const source=el('chat-source-mode').value
  if(source==='desktop_ui'){note('微信界面读取需在更多选项中指定会话。');return}
  const result=await call('desktop_chat_source_conversations',{source,cursor:more?cursor:null})
  if(!Array.isArray(result.chats))throw Error('invalid')
  const combined=more?[...chats,...result.chats]:result.chats
  chats=[...new Map(combined.map(c=>[c.chat_id,c])).values()];cursor=result.has_more?result.next_cursor:null
  loaded=true;renderChats();note(chats.length?`已加载 ${chats.length} 个聊天，请选择后读取片段。${cursor?'未找到时可加载更多。':''}`:'此来源暂无聊天，可切换读取来源或手动粘贴。')
 }
 async function searchChats(){
  if(pending||blocked)return
  const term=el('chat-source-search').value.trim().toLocaleLowerCase()
  if(!term){renderChats();return}
  if(!loaded){await check();if(!loaded)return}
  reset();el('chat-source-id').value='';pending=true;onBusy(true);controls()
  try{
   // A slow data source must not trigger a full-history enumeration from a keystroke.
   // Search at most five extra pages per explicit action; the user can continue.
   const seen=new Set();const deadline=Date.now()+30000
   while(cursor&&Date.now()<deadline&&seen.size<5){
    if(seen.has(cursor))throw Error('repeated cursor')
    seen.add(cursor);note(`正在查找聊天…已检查 ${chats.length} 个`)
    await loadChats(true)
   }
   renderChats()
   const matches=chats.filter(c=>(c.title||c.chat_id).toLocaleLowerCase().includes(term))
   if(!cursor&&matches.length===1){
    el('chat-source-id').value=matches[0].chat_id;el('chat-source-conversation').value=matches[0].chat_id
    note(`已找到“${matches[0].title||'聊天'}”，点击读取片段。`)
   }else note(matches.length?`找到 ${matches.length} 个聊天，请从列表选择。${cursor?'仍有会话未检查，点击继续查找。':''}`:
    cursor?'暂未找到，仍有会话未检查。点击继续查找。':'当前数据来源中未找到该聊天，请尝试名称中的几个字。')
   el('chat-source-more').textContent=cursor?'继续查找':'加载更多聊天'
  }catch{note('查找未完成，请点击“选择微信聊天”重试。')}
  finally{pending=false;onBusy(false);controls()}
 }
 async function refresh(more=false){
  if(pending||blocked)return
  reset();pending=true;onBusy(true);controls();note('正在加载聊天…')
  try{await loadChats(more)}catch{note('聊天列表加载失败，请重试；仍可手动粘贴。')}
  finally{pending=false;onBusy(false);controls()}
 }
 async function check(){
  if(pending||blocked)return
  reset();chats=[];cursor=null;el('chat-source-id').value='';renderChats()
  pending=true;onBusy(true);controls()
  try{
   const state=await call('desktop_chat_source_status')
   const live=state.read_contract_supported&&state.live_read_available
   const structured=state.read_contract_supported&&state.structured_db_read
   el('chat-source-mode').querySelector('[value=desktop_ui]').disabled=!live
   el('chat-source-mode').querySelector('[value=structured_db]').disabled=!structured
   if((!live&&el('chat-source-mode').value==='desktop_ui')||
      (!structured&&el('chat-source-mode').value==='structured_db'))el('chat-source-mode').value='local_index'
   if(structured)el('chat-source-mode').value='structured_db'
   if(state.read_contract_supported){await loadChats()}
   else note(!state.registered?'请到设置 → 微信连接已安装的 Provider。':'请升级 Provider 后再选择聊天。')
  }catch(error){note('聊天列表加载失败：'+String(error).slice(0,240))}
  finally{pending=false;onBusy(false);controls()}
 }
 async function read(){
  if(pending||blocked)return
  const chatId=el('chat-source-id').value.trim();if(!chatId){note('请先从列表选择聊天。');return}
  reset();pending=true;onBusy(true);controls();note('正在读取指定会话片段…')
  try{
   data=await call('desktop_chat_source_read',{chatId,source:el('chat-source-mode').value,limit:10})
   if(!Array.isArray(data.messages))throw Error('invalid')
   const sourceLabel=data.source==='local_index'?'已有索引 · 非实时':
    data.source==='structured_db'?'本机结构化库 · 按需读取':'微信界面 · 可见范围'
   const identityNote=data.id_kind==='stable_conversation_id'?'请核对片段后使用。':'显示名不是稳定身份，请核对内容。'
   note(`${sourceLabel} · ${chats.find(c=>c.chat_id===data.chat_id)?.title||data.chat_id} · ${data.messages.length} 条 · 不完整${data.cached_fallback?'；实时读取失败，返回的是旧索引':''}${data.skipped_non_text?'；跳过非文本 '+data.skipped_non_text+' 条':''}。 ${identityNote}`)
   for(const [i,m] of data.messages.entries()){
    const label=document.createElement('label');label.className='chat-source-message'
    const checkbox=document.createElement('input');checkbox.type='checkbox';checkbox.value=String(i);checkbox.checked=true
    const body=document.createElement('span'),meta=document.createElement('small'),text=document.createElement('span')
    meta.textContent=`${m.sender||'发言人未知'} · ${m.source||'来源未知'}${m.sent_at?' · '+m.sent_at:''}${m.imported_at?' · 入库 '+m.imported_at:''}`
    text.textContent=m.text;body.append(meta,text);label.append(checkbox,body);el('chat-source-messages').append(label)
   }
   if(!data.messages.length)data=null
  }catch(error){
   reset()
   const reason=String(error)
   note(reason.includes('微信数据源访问被 macOS 拒绝')?
    reason.slice(0,650):'片段读取未完成。实时功能可能受限或 Provider 需要升级；可改选已有索引或手动粘贴。未自动重试。')
  }
  finally{pending=false;onBusy(false);controls()}
 }
 el('chat-source-check').addEventListener('click',()=>void check())
 el('chat-source-read').addEventListener('click',()=>void read())
 el('chat-source-apply').addEventListener('click',()=>{
  if(pending||blocked||!data)return
  try{
   const indexes=[...el('chat-source-messages').querySelectorAll('input:checked')].map(c=>Number(c.value))
   const name=chats.find(c=>c.chat_id===data.chat_id)?.title
   onUse(selectedContext(data,indexes,name));note('所选片段已放入本轮输入。核对后点击“生成建议”才会交给 Agent。')
  }catch(error){note(String(error))}
 })
 el('chat-source-search').addEventListener('input',()=>{reset();el('chat-source-id').value='';controls();renderChats();clearTimeout(searchTimer);searchTimer=setTimeout(()=>void searchChats(),400)})
 el('chat-source-conversation').addEventListener('change',()=>{reset();el('chat-source-id').value=el('chat-source-conversation').value;controls()})
 el('chat-source-more').addEventListener('click',()=>void (el('chat-source-search').value.trim()?searchChats():refresh(true)))
 el('chat-source-id').addEventListener('input',()=>reset())
 el('chat-source-mode').addEventListener('change',()=>{chats=[];cursor=null;el('chat-source-id').value='';renderChats();void refresh()})
 controls()
 return {setBlocked(value){blocked=value;controls()},reset(){reset();note('本轮读取片段已清除；未读取新消息。')}}
}