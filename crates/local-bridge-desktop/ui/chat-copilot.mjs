import {mountChatSource} from './chat-source.mjs'
// No storage, telemetry, HTML interpolation or connector calls in this module.
export const FEEDBACK=['自然','太油','太冷','不像我','越界']
// User-selected context, not a diagnosis of the other person's feelings or intentions.
export const RELATIONSHIP_HINTS=Object.freeze({
  general:'仅根据聊天判断，不假设双方关系。',
  friends:'朋友或同学：自然接话，有共同梗才用幽默，不硬加暧昧。',
  work:'同事或客户：专业清晰，避免擅自承诺时间、交付或立场。',
  family:'家人与亲友：先理解感受，避免说教或替对方决定。',
  new_connection:'新认识或互相了解：低压力、不把礼貌当明确好感。',
  partner:'伴侣或亲密关系：优先真诚、修复和双方边界，不操控情绪。',
  group:'多人群聊：把观点与发言人分开，不推断群成员的心理。'
})
export function relationshipHint(value){return RELATIONSHIP_HINTS[value]||RELATIONSHIP_HINTS.general}
const bounded=(v,max)=>typeof v==='string'&&v.trim().length>0&&v.length<=max
export function parseDraft(raw){
  if(typeof raw!=='string'||raw.length>32000)throw new Error('结果为空或超出显示限制')
  let source=raw.trim(),commentary=''
  const fenced=source.match(/^```(?:json)?\s*\n?([\s\S]*?)\n?```([\s\S]*)$/)
  if(fenced){
    source=fenced[1];commentary=fenced[2].trim()
    if(commentary.length>2000||commentary.includes('```'))throw new Error('结果包含多个区块或过长说明，请检查原始草稿')
  }
  let data
  try{data=JSON.parse(source)}catch{throw new Error('结果不是有效 JSON，请检查原始草稿')}
  if(!data||!bounded(data.judgment,2000)||!bounded(data.avoid,2000)||
    !['facts','hypotheses','unknowns'].every(k=>Array.isArray(data[k])&&data[k].length<=12&&data[k].every(v=>bounded(v,2000)))||
    !Array.isArray(data.candidates)||data.candidates.length<2||data.candidates.length>3||
    !data.candidates.every(c=>c&&bounded(c.label,40)&&bounded(c.text,2000)))
    throw new Error('结果结构不完整，请检查原始草稿')
  return commentary?{...data,commentary}:data
}
export async function copyDraft(text,clipboard){
  if(!text.trim())throw new Error('草稿为空')
  if(!clipboard?.writeText)throw new Error('剪贴板不可用，请选中文字手动复制')
  await clipboard.writeText(text)
}
export async function readUntilDone(call,id,{delay=()=>new Promise(r=>setTimeout(r,800)),limit=240}={}){
  for(let n=0;n<limit;n++){
    const result=await call('desktop_chat_read',{id})
    if(result.status==='completed')return result
    if(result.status==='failed')throw new Error(result.error||'Agent 未完成')
    if(result.status!=='running')throw new Error('Agent 状态未知；未自动重试')
    await delay()
  }
  throw new Error('等待超时；未自动重试。可点击“读取本轮结果”继续查询')
}

export function mountChat({call,document,clipboard}){
  const el=id=>document.getElementById(id)
  let selection=null,busy=false,sourceBusy=false,taskId=null,feedback='',sourceContext={}
  let sourceView
  const say=text=>{el('chat-status').textContent=text}
  function refreshRelationshipHint(){el('chat-relationship-hint').textContent=relationshipHint(el('chat-relationship').value)}
  const node=(tag,text,cls='')=>{
    const element=document.createElement(tag);element.textContent=text; if(cls)element.className=cls;return element
  }
  function controls(){
    el('chat-generate').disabled=busy||sourceBusy||!selection||taskId!==null
    el('chat-clear').disabled=busy||sourceBusy
    el('chat-read').hidden=taskId===null
    el('chat-read').disabled=busy
    el('chat-refresh-agent').disabled=busy||sourceBusy||taskId!==null
    el('chat-inputs').disabled=busy||sourceBusy||taskId!==null
    sourceView?.setBlocked(busy||taskId!==null)
  }
  async function refresh(){
    if(busy||sourceBusy||taskId!==null)return
    selection=null;controls()
    try{
      selection=await call('desktop_chat_selection')
      const compatibility=selection.agent==='codex'?'此入口暂不支持 Codex 自定义服务配置。':selection.agent==='pi'?'此入口暂不支持 Pi 扩展模型。':''
      el('chat-agent').textContent=`${selection.agent} · ${selection.model} · ${selection.think_mode}`
      el('chat-disclosure').textContent=`${compatibility}本次使用 ${selection.agent} · 模型 ${selection.model} · 思考 ${selection.think_mode}。点击生成会将选定片段、意图、风格与本轮反馈交给该 Agent，可能发送至其远端模型。Bridge 不保存聊天原文；Agent 服务的数据处理遵循其自身规则。`
    }catch(error){el('chat-agent').textContent='Agent 暂不可用';el('chat-disclosure').textContent=String(error)+'。仍可编辑或复制已有文字。'}
    controls()
  }
  function render(raw){
    const area=el('chat-results');area.replaceChildren()
    let data
    try{data=parseDraft(raw)}catch(error){
      area.append(node('p',String(error),'muted-banner'))
      const field=document.createElement('textarea');field.value=String(raw||'').slice(0,32000);field.rows=10
      field.setAttribute('aria-label','未解析的原始草稿');area.append(field)
      area.append(copyButton(field));return
    }
    area.append(node('p',data.judgment,'chat-judgment'))
    for(const [index,candidate] of data.candidates.entries()){
      const card=node('article','', 'chat-draft'),heading=node('div','','chat-draft-heading')
      const label=node('label',candidate.label);label.htmlFor='chat-draft-'+index
      const field=document.createElement('textarea');field.value=candidate.text;field.rows=2;field.id=label.htmlFor
      field.setAttribute('aria-label',candidate.label+'草稿')
      heading.append(label,copyButton(field));card.append(heading,field);area.append(card)
    }
    const analysis=node('details','','chat-analysis')
    analysis.append(node('summary','查看依据 · 事实 / 推测 / 未知'))
    const columns=node('div','','chat-analysis-grid')
    for(const [key,label] of [['facts','事实'],['hypotheses','推测'],['unknowns','未知 / 待澄清']]){
      const group=node('section','');group.append(node('h4',label))
      const list=document.createElement('ul')
      for(const text of data[key])list.append(node('li',text))
      if(!data[key].length)list.append(node('li','未提供'))
      group.append(list);columns.append(group)
    }
    analysis.append(columns);area.append(analysis)
    area.append(node('p','注意：'+data.avoid,'muted-banner'))
    if(data.commentary)area.append(node('p','补充：'+data.commentary,'muted-banner'))
    const buttons=node('div','','actions chat-feedback')
    buttons.append(node('span','像你吗？','chat-muted'))
    for(const value of FEEDBACK){
      const button=node('button',value,'button secondary');button.type='button'
      button.addEventListener('click',()=>{
        feedback=value
        for(const child of buttons.children)child.setAttribute('aria-pressed',String(child===button))
        say('本轮反馈：'+value+'。下次生成时使用；清除或关闭页面后移除。')
      });buttons.append(button)
    }
    area.append(buttons)
  }
  function copyButton(field){
    const button=node('button','复制文字','button secondary');button.type='button'
    button.addEventListener('click',async()=>{
      try{await copyDraft(field.value,clipboard);say('已复制修改后的文字，请回原应用自行发送。')}
      catch{field.focus();field.select();say('复制失败，已选中文字，请手动复制。')}
    });return button
  }
  async function read(){
    if(taskId===null||busy)return
    busy=true;controls();say('正在等待 '+selection.agent+' 的本轮结果…')
    try{
      const result=await readUntilDone(call,taskId)
      render(result.result);taskId=null;say('Agent 已完成。请核对事实、修改草稿，再复制。')
      await call('desktop_chat_clear') // UI retains the visible drafts only.
    }catch(error){
      say(String(error))
      // A failed/unknown transport is never a reason to submit another task.
      try{const state=await call('desktop_chat_read',{id:taskId});if(state.status==='failed')taskId=null}catch{}
    }finally{busy=false;controls()}
  }
  el('chat-form').addEventListener('submit',async event=>{
    event.preventDefault();if(busy||sourceBusy||!selection||taskId!==null)return
    taskId=globalThis.crypto.randomUUID()
    busy=true;controls();el('chat-results').replaceChildren();say('正在提交本轮片段…')
    try{
      const result=await call('desktop_chat_run',{input:{...selection,...sourceContext,client_key:taskId,mode:el('chat-mode').value,text:el('chat-text').value,
        intent:el('chat-intent').value,voice:el('chat-voice').value,relationship:el('chat-relationship').value,
        constraints:el('chat-constraints').value,feedback}})
      if(result.status==='rejected'){
        taskId=null;say(result.error+'。本轮未提交，修改后可重新生成。');return
      }
      taskId=result.task_id
    }catch(error){say(String(error)+'。可读取本轮结果，或清除后重试。');busy=false;controls();return}
    finally{busy=false;controls()}
    if(taskId!==null)await read()
  })
  el('chat-clear').addEventListener('click',async()=>{
    try{
      await call('desktop_chat_clear');taskId=null;feedback='';sourceContext={};sourceView.reset();el('chat-context-source').textContent='手动粘贴';el('chat-form').reset();el('chat-results').replaceChildren()
      say('本轮输入、草稿和反馈已清除。剪贴板由系统管理。');controls()
    }catch(error){say(String(error))}
  })
  el('chat-no-reply').addEventListener('click',()=>say('本轮选择不回复。不会发送消息；需要时可清除本轮内容。'))
  el('chat-read').addEventListener('click',()=>void read())
  el('chat-refresh-agent').addEventListener('click',()=>void refresh())
  sourceView=mountChatSource({document,call,onBusy(value){sourceBusy=value;controls()},onUse(context){
    sourceContext={context_source:context.context_source,thread_label:context.thread_label}
    el('chat-text').value=context.text
    el('chat-context-source').textContent=context.context_source==='wechat_local_index'?'微信索引 · 非实时':context.context_source==='wechat_structured_db'?'微信本地结构化库':'微信可见片段'
    say('已载入 '+context.thread_label+' 的所选片段；尚未发送给 Agent。')
  }})
  el('chat-text').addEventListener('input',()=>{sourceContext={};el('chat-context-source').textContent='手动编辑'})
  el('chat-relationship').addEventListener('change',refreshRelationshipHint)
  refreshRelationshipHint()
  return {refresh}
}