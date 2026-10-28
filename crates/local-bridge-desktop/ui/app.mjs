import {AGENTS,AGENT_LABELS,THINK_OPTIONS,defaults,shiftPriority,normalizePreferenceDraft} from './preferences.mjs'

const invoke=globalThis.__TAURI__?.core?.invoke
const el=id=>document.getElementById(id)
const pageLabels={overview:'概览',tasks:'任务中心',settings:'Agent 设置'}
const state={prefs:defaults(),tasks:null}

function node(tag,text,cls=''){
  const item=document.createElement(tag)
  if(cls)item.className=cls
  item.textContent=String(text??'')
  return item
}
function notice(message,success=false){
  const target=el('notice')
  target.hidden=!message
  target.className=success?'notice success':'notice'
  target.textContent=message||''
}
async function call(name,args={}){
  if(typeof invoke!=='function')
    throw new Error('此界面需要通过 Tauri 桌面程序启动；浏览器预览无本机权限')
  return invoke(name,args)
}
const states={
  running:['运行中','warn'],queued:['排队中',''],
  needs_action:['待处理','warn'],unknown:['状态未知','bad'],
  completed:['已完成','ok'],failed:['失败','bad'],
  not_installed:['未安装',''],unverified:['未验证','warn'],
  unavailable:['不可用','bad']
}
function pill(value){
  const [name,type]=states[value]||[value||'未知','']
  return node('span',name,'pill '+type)
}
function metric(label,value,description){
  const card=node('div',null,'stat')
  card.append(node('div',label,'stat-name'),node('div',value,'stat-value'),
    node('div',description,'stat-note'))
  return card
}
function line(label,description,status){
  const wrapper=node('div',null,'item-row')
  const left=document.createElement('div')
  left.append(node('div',label,'item-title'),node('div',description,'item-sub'))
  wrapper.append(left,pill(status))
  return wrapper
}
async function loadOverview(){
  notice('')
  const button=el('refresh-overview');button.disabled=true
  try{
    const data=await call('desktop_overview')
    el('version-label').textContent='Desktop '+(data.version||'预览版')
    const checks=data.diagnostics?.checks||[]
    const agents=data.agents||[]
    const providers=data.mcp_providers?.providers||[]
    el('overview-stats').replaceChildren(
      metric('本机基础诊断',data.diagnostics?.ok===true?'正常':'需检查',
        String(checks.filter(x=>x.ok===false).length)+' 项异常'),
      metric('已发现 Agent',String(agents.filter(x=>x.installed).length)+' / 3',
        '已安装不代表已登录'),
      metric('MCP Provider',String(providers.length),'已注册不代表授权通过')
    )
    const list=el('overview-agents');list.replaceChildren()
    for(const agent of agents){
      list.append(line(AGENT_LABELS[agent.agent]||agent.agent,
        (agent.run_supported?'支持当前平台':'当前平台未支持')+' · 登录/API 尚未核验',
        agent.installed?'unverified':'not_installed'))
    }
    const components=el('overview-components');components.replaceChildren()
    const host=data.host||{}
    components.append(line('Bridge Chrome Native Host','仅 com.eaopen.browser_bridge',
      host.state==='manifest_ready_browser_not_verified'?'unverified':
      host.state==='not_installed'?'not_installed':'unavailable'))
    if(!providers.length){
      components.append(line('MCP Provider','尚未配置可选 Provider','not_installed'))
    }else{
      for(const p of providers){
        components.append(line(p.id||'Provider','仅本机注册状态，未检查实际登录','unverified'))
      }
    }
  }catch(error){notice('读取概览失败：'+error)}
  finally{button.disabled=false}
}
function when(time){
  if(typeof time!=='number'||!Number.isFinite(time)||time<=0)return '—'
  return new Date(time).toLocaleString('zh-CN',{month:'2-digit',day:'2-digit',
    hour:'2-digit',minute:'2-digit'})
}
function renderTasks(){
  if(!state.tasks)return
  const data=state.tasks
  const sources=el('source-health');sources.replaceChildren()
  for(const source of data.sources||[]){
    const ok=source.state==='ok'||source.state==='not_requested'
    const label=source.source+' · '+(source.state||'未知')+
      (source.truncated?' · 部分结果':'')
    sources.append(node('span',label,'source-tag'+(ok?'':' error')))
  }
  el('tasks-alert').hidden=data.complete===true
  el('tasks-alert').textContent=data.complete===true?'':'部分任务来源不可用或结果被截断；未将其当作已完成。'
  const filter=el('task-filter').value
  const items=(data.items||[]).filter(x=>filter==='all'||x.status===filter)
  const body=el('task-body');body.replaceChildren()
  if(!items.length){
    const tr=document.createElement('tr'),td=node('td','没有符合条件的任务','empty')
    td.colSpan=4;tr.append(td);body.append(tr);return
  }
  for(const task of items){
    const tr=document.createElement('tr')
    const first=document.createElement('td')
    first.append(node('div',task.title||'任务','task-title'),
      node('div',String(task.task_id||'').slice(0,18),'task-id'))
    tr.append(first,node('td',task.provider||task.owner_module||'—'))
    const status=document.createElement('td');status.append(pill(task.status));tr.append(status)
    tr.append(node('td',when(task.created_at_unix_ms)))
    body.append(tr)
  }
}
async function loadTasks(){
  notice('')
  const button=el('refresh-tasks');button.disabled=true
  button.textContent='正在读取…'
  try{
    const includeCloudfile=el('include-cloudfile').checked
    state.tasks=await call('desktop_tasks',{includeCloudfile})
    renderTasks()
  }catch(error){notice('任务读取失败：'+error)}
  finally{button.disabled=false;button.textContent='刷新列表'}
}
function fillThinkMode(){
  const name=el('setting-agent').value
  const select=el('setting-think')
  const previous=select.value||state.prefs.think_mode||'auto'
  select.replaceChildren()
  for(const mode of THINK_OPTIONS[name]||['auto']){
    const option=node('option',mode==='auto'?'自动':mode)
    option.value=mode;select.append(option)
  }
  select.value=THINK_OPTIONS[name]?.includes(previous)?previous:'auto'
  const isAuto=name==='auto',isWorkbuddy=name==='workbuddy'
  el('setting-agent-mode').disabled=isAuto
  if(isAuto)el('setting-agent-mode').value='prefer'
  el('setting-model').disabled=isWorkbuddy
  if(isWorkbuddy)el('setting-model').value='auto'
}
function renderPriority(){
  const list=el('priority-list');list.replaceChildren()
  const priority=state.prefs.priority
  for(const [i,agent] of priority.entries()){
    const item=node('div',null,'priority-item')
    item.append(node('span',String(i+1),'priority-number'),
      node('span',AGENT_LABELS[agent]||agent,'priority-name'))
    for(const [direction,title] of [[-1,'上移'],[1,'下移']]){
      const button=node('button',title);button.type='button'
      button.disabled=i+direction<0||i+direction>=priority.length
      button.addEventListener('click',()=>{
        state.prefs.priority=shiftPriority(priority,agent,direction)
        renderPriority()
      })
      item.append(button)
    }
    list.append(item)
  }
}
function putPreferences(preferences){
  state.prefs={...defaults(),...preferences,
    priority:Array.isArray(preferences.priority)?[...preferences.priority]:[...AGENTS]}
  el('setting-agent').value=state.prefs.agent
  el('setting-agent-mode').value=state.prefs.agent_mode
  el('setting-model').value=state.prefs.model
  fillThinkMode()
  el('setting-think').value=THINK_OPTIONS[state.prefs.agent]?.includes(state.prefs.think_mode)?
    state.prefs.think_mode:'auto'
  renderPriority()
}
async function loadPreferences(){
  notice('')
  try{putPreferences(await call('desktop_preferences_get'))}
  catch(error){notice('设置读取失败：'+error)}
}
async function savePreferences(event){
  event.preventDefault()
  const button=el('save-preferences');button.disabled=true
  try{
    const draft=normalizePreferenceDraft({
      schema_version:1,agent:el('setting-agent').value,
      agent_mode:el('setting-agent-mode').value,
      model:el('setting-model').value,think_mode:el('setting-think').value,
      priority:state.prefs.priority
    })
    putPreferences(await call('desktop_preferences_save',{preferences:draft}))
    notice('已保存到本机。仅对未明确指定参数的新任务生效。',true)
  }catch(error){notice('保存失败：'+error)}
  finally{button.disabled=false}
}
function setPage(page){
  if(!pageLabels[page])return
  el('page-heading').textContent=pageLabels[page]
  for(const key of Object.keys(pageLabels))el('page-'+key).hidden=key!==page
  for(const button of document.querySelectorAll('[data-page]')){
    const selected=button.dataset.page===page
    button.classList.toggle('selected',selected)
    if(selected)button.setAttribute('aria-current','page')
    else button.removeAttribute('aria-current')
  }
  notice('')
  if(page==='overview')void loadOverview()
  if(page==='tasks')void loadTasks()
  if(page==='settings')void loadPreferences()
}
for(const button of document.querySelectorAll('[data-page]')){
  button.addEventListener('click',()=>setPage(button.dataset.page))
}
el('refresh-overview').addEventListener('click',()=>void loadOverview())
el('refresh-tasks').addEventListener('click',()=>void loadTasks())
el('task-filter').addEventListener('change',renderTasks)
el('include-cloudfile').addEventListener('change',()=>{
  el('tasks-alert').hidden=false
  el('tasks-alert').textContent='已改变查询范围；点击“刷新列表”应用。'
})
el('setting-agent').addEventListener('change',fillThinkMode)
el('preferences-form').addEventListener('submit',savePreferences)
el('reset-preferences').addEventListener('click',()=>{
  putPreferences(defaults())
  notice('已恢复表单默认值。点击“保存到本机”后才会生效。')
})
void loadOverview()
