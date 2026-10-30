// Explicit local-user reads only. No inference, sends, auto polling or persistence.
export function selectedContext(data,indexes){
 if(!['local_index','desktop_ui'].includes(data?.source)||typeof data.chat_id!=='string'||!Array.isArray(data.messages))throw new Error('来源无效')
 const text=indexes.map(i=>{const m=data.messages[i];if(!m||typeof m.text!=='string')throw new Error('片段无效');return `${m.sender||'发言人未知'}：${m.text}`}).join('\n')
 if(!text.trim()||new TextEncoder().encode(text).length>7800)throw new Error('请选择少量片段，总长度需小于 7800 字节。')
 return {text,context_source:'wechat_'+data.source,thread_label:data.chat_id}
}
export function mountChatSource({document,call,onBusy,onUse}){
 const el=id=>document.getElementById(id)
 let pending=false,blocked=false,data=null
 const note=text=>{el('chat-source-status').textContent=text}
 function controls(){el('chat-source-fields').disabled=pending||blocked;el('chat-source-apply').disabled=pending||blocked||!data}
 function reset(){data=null;el('chat-source-messages').replaceChildren();controls()}
 async function check(){
  if(pending||blocked)return
  pending=true;onBusy(true);controls()
  try{
   const state=await call('desktop_chat_source_status')
   const live=state.read_contract_supported&&state.live_read_available
   el('chat-source-mode').querySelector('[value=desktop_ui]').disabled=!live
   if(!live)el('chat-source-mode').value='local_index'
   note(!state.registered?'尚未注册 Provider，请到设置 → 微信，填写已安装的可执行文件路径。':
    !state.read_contract_supported?'协议已响应；请升级 Provider，当前版本不支持明确区分读取来源。':
    live?'Provider 已响应。实时读取可尝试，但尚未完成此设备的实机验收。':
    state.blocked_reason==='macos_login_safety_hold'?'Provider 已响应；仅可读取已有索引。Mac 实时操作因登录保护暂停，不会触碰微信。':
    'Provider 已响应；实时后端尚不可用，可读取已有索引或手动粘贴。')
  }catch{note('Provider 连接检查失败。可继续手动粘贴；未尝试读取微信。')}
  finally{pending=false;onBusy(false);controls()}
 }
 async function read(){
  if(pending||blocked)return
  const chatId=el('chat-source-id').value.trim();if(!chatId){note('请填写你要读取的会话显示名。');return}
  reset();pending=true;onBusy(true);controls();note('正在读取指定会话片段…')
  try{
   data=await call('desktop_chat_source_read',{chatId,source:el('chat-source-mode').value,limit:10})
   if(!Array.isArray(data.messages))throw Error('invalid')
   note(`${data.source==='local_index'?'已有索引 · 非实时':'微信界面 · 可见范围'} · ${data.chat_id} · ${data.messages.length} 条 · 不完整${data.cached_fallback?'；实时读取失败，返回的是旧索引':''}。显示名不是稳定身份，请核对内容。`)
   for(const [i,m] of data.messages.entries()){
    const label=document.createElement('label');label.className='chat-source-message'
    const checkbox=document.createElement('input');checkbox.type='checkbox';checkbox.value=String(i);checkbox.checked=true
    const body=document.createElement('span'),meta=document.createElement('small'),text=document.createElement('span')
    meta.textContent=`${m.sender||'发言人未知'} · ${m.source||'来源未知'}${m.sent_at?' · '+m.sent_at:''}${m.imported_at?' · 入库 '+m.imported_at:''}`
    text.textContent=m.text;body.append(meta,text);label.append(checkbox,body);el('chat-source-messages').append(label)
   }
   if(!data.messages.length)data=null
  }catch{reset();note('片段读取未完成。实时功能可能受限或 Provider 需要升级；可改选已有索引或手动粘贴。未自动重试。')}
  finally{pending=false;onBusy(false);controls()}
 }
 el('chat-source-check').addEventListener('click',()=>void check())
 el('chat-source-read').addEventListener('click',()=>void read())
 el('chat-source-apply').addEventListener('click',()=>{
  if(pending||blocked||!data)return
  try{
   const indexes=[...el('chat-source-messages').querySelectorAll('input:checked')].map(c=>Number(c.value))
   onUse(selectedContext(data,indexes));note('所选片段已放入本轮输入。核对后点击“生成建议”才会交给 Agent。')
  }catch(error){note(String(error))}
 })
 for(const id of ['chat-source-id','chat-source-mode'])el(id).addEventListener('input',()=>reset())
 controls()
 return {setBlocked(value){blocked=value;controls()},reset(){reset();note('本轮读取片段已清除；未读取新消息。')}}
}
