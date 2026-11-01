// Explicit local-user reads only. No inference, sends, auto polling or persistence.
// A provider is user-installed; Bridge never hardcodes one vendor's message logic.
const PROVIDERS={
 wechat:{sources:['local_index','desktop_ui','structured_db'],preference:['structured_db','local_index','desktop_ui']},
 soul:{sources:['web_page'],preference:['web_page']}
}
const SOURCE_LABELS={
 local_index:'已有索引 · 非实时',
 desktop_ui:'聊天界面 · 可见范围',
 structured_db:'本机结构化库 · 按需读取',
 web_page:'网页会话 · 用户可见范围'
}
export function providerSources(provider){
 const config=PROVIDERS[provider]
 if(!config)throw new Error('未知的聊天来源')
 return config.sources
}
export function selectedContext(data,indexes,displayName,provider='wechat'){
 const allowed=providerSources(provider)
 if(!allowed.includes(data?.source)||typeof data.chat_id!=='string'||!Array.isArray(data.messages))throw new Error('来源无效')
 if(typeof data.provider==='string'&&data.provider!==provider)throw new Error('来源无效')
 if(data.source==='web_page'&&(data.live_read!==true||data.session_visible_only!==true))throw new Error('来源无效')
 const text=indexes.map(i=>{const m=data.messages[i];if(!m||typeof m.text!=='string')throw new Error('片段无效');return `${m.sender||'发言人未知'}：${m.text}`}).join('\n')
 if(!text.trim()||new TextEncoder().encode(text).length>7800)throw new Error('请选择少量片段，总长度需小于 7800 字节。')
 const label=typeof displayName==='string'&&displayName.trim()&&displayName.length<=200?
  displayName.trim():data.chat_id
 return {text,context_source:`${provider}_${data.source}`,thread_label:label}
}
export function mountChatSource({document,call,onBusy,onUse}){
 const el=id=>document.getElementById(id)
 let pending=false,blocked=false,data=null,chats=[],cursor=null,searchTimer=null,loaded=false,readable=false,dataProvider=null
 const note=text=>{el('chat-source-status').textContent=text}
 const providerOf=()=>el('chat-source-provider')?.value||'wechat'
 const modeOf=()=>el('chat-source-mode').value
 const configFor=provider=>PROVIDERS[provider]||PROVIDERS.wechat
 const isWebPage=()=>modeOf()==='web_page'
 function controls(){
  el('chat-source-provider').disabled=pending||blocked
  el('chat-source-read').disabled=pending||blocked||!readable||!el('chat-source-id').value.trim()
  el('chat-source-fields').disabled=pending||blocked
  el('chat-source-apply').disabled=pending||blocked||!data
 }
 function reset(){data=null;dataProvider=null;el('chat-source-messages').replaceChildren();controls()}
 function syncListFields(){
  const list=el('chat-source-list-fields')
  if(list)list.hidden=isWebPage()||!readable
 }
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
 async function loadChats(more=false,owner=providerOf()){
  const source=modeOf()
  if(source==='desktop_ui'){note('界面读取需在更多选项中指定会话。');return}
  const result=await call('desktop_chat_source_conversations',{provider:owner,source,cursor:more?cursor:null})
  if(owner!==providerOf())return
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
  }catch{note('查找未完成，请重新检查聊天来源。')}
  finally{pending=false;onBusy(false);controls()}
 }
 async function refresh(more=false){
  if(pending||blocked)return
  const owner=providerOf()
  reset();pending=true;onBusy(true);controls();note('正在加载聊天…')
  try{await loadChats(more,owner)}catch{note('聊天列表加载失败，请重试；仍可手动粘贴。')}
  finally{pending=false;onBusy(false);controls()}
 }
 async function check(){
  if(pending||blocked)return
  const requested=providerOf()
  const sources=configFor(requested).sources
  reset();chats=[];cursor=null;loaded=false;readable=false;el('chat-source-id').value='';renderChats()
  pending=true;onBusy(true);controls()
  try{
   const state=await call('desktop_chat_source_status',{provider:requested})
   if(providerOf()!==requested)return
   const contract=state.read_contract_supported===true
   const capability={
    local_index:contract&&state.local_index_read===true,
    desktop_ui:contract&&state.live_read_available===true,
    structured_db:contract&&state.structured_db_read===true,
    web_page:contract&&state.web_page_read===true
   }
   const select=el('chat-source-mode')
   for(const value of ['local_index','structured_db','desktop_ui','web_page']){
    const option=select.querySelector('[value='+value+']')
    if(option)option.disabled=!sources.includes(value)||!capability[value]
   }
   const usable=sources.filter(value=>capability[value])
   readable=usable.length>0
   if(usable.length){
    const preferred=configFor(requested).preference.find(value=>usable.includes(value))
    if(preferred)select.value=preferred
   }
   syncListFields()
   if(!contract){
    note(state.registered===false?'尚未安装或注册该聊天来源 Provider；仍可手动粘贴。':'该 Provider 需要升级后再选择聊天；仍可手动粘贴。')
   }else if(!usable.length){
    note('该 Provider 未声明当前来源的读取能力；可改用手动粘贴。')
   }else if(isWebPage()){
    note('请填写你在网页中可见的会话名称，再读取片段；不会自动翻页、监听或发送。')
   }else{
    await loadChats(false,requested)
   }
  }catch(error){note('聊天来源检查失败：'+String(error).slice(0,240))}
  finally{pending=false;onBusy(false);controls()}
 }
 async function read(){
  if(pending||blocked)return
  if(!readable){note('当前来源不可读取；请先检查聊天来源，或改用手动粘贴。');return}
  const requested=providerOf()
  const chatId=el('chat-source-id').value.trim();if(!chatId){note('请先选择或填写会话名称。');return}
  const source=modeOf()
  reset();pending=true;onBusy(true);controls();note('正在读取指定会话片段…')
  try{
   data=await call('desktop_chat_source_read',{provider:requested,chatId,source,limit:10})
   if(providerOf()!==requested)return
   if(!Array.isArray(data.messages))throw Error('invalid')
   dataProvider=typeof data.provider==='string'?data.provider:requested
   const sourceLabel=SOURCE_LABELS[data.source]||'来源未标注'
   const identityNote=data.id_kind==='stable_conversation_id'?'请核对片段后使用。':
    data.source==='web_page'?'仅当前可见内容，可能有遗漏；请核对后使用。':'显示名不是稳定身份，请核对内容。'
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
    reason.slice(0,650):'片段读取未完成。可能未登录、能力未声明或 Provider 需要升级；可改用手动粘贴。未自动重试。')
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
   const owner=typeof data.provider==='string'?data.provider:(dataProvider||providerOf())
   onUse(selectedContext(data,indexes,name,owner));note('所选片段已放入本轮输入。核对后点击“生成建议”才会交给 Agent。')
  }catch(error){note(String(error))}
 })
 el('chat-source-search').addEventListener('input',()=>{reset();el('chat-source-id').value='';controls();renderChats();clearTimeout(searchTimer);searchTimer=setTimeout(()=>void searchChats(),400)})
 el('chat-source-conversation').addEventListener('change',()=>{reset();el('chat-source-id').value=el('chat-source-conversation').value;controls()})
 el('chat-source-more').addEventListener('click',()=>void (el('chat-source-search').value.trim()?searchChats():refresh(true)))
 el('chat-source-id').addEventListener('input',()=>reset())
 el('chat-source-mode').addEventListener('change',()=>{syncListFields();chats=[];cursor=null;el('chat-source-id').value='';renderChats();if(isWebPage())note('请填写你在网页中可见的会话名称，再读取片段；不会自动翻页、监听或发送。');else void refresh()})
 el('chat-source-provider')?.addEventListener('change',()=>void check())
 controls()
 return {setBlocked(value){blocked=value;controls()},reset(){reset();note('本轮读取片段已清除；未读取新消息。')}}
}
