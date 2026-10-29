import {AGENTS,AGENT_LABELS,THINK_OPTIONS,defaults,shiftPriority,normalizePreferenceDraft} from './preferences.mjs'
import {SETTINGS,generalPreferences,connectionState} from './settings.mjs'

const invoke=globalThis.__TAURI__?.core?.invoke
const el=id=>document.getElementById(id)
const pageLabels={overview:'概览',tasks:'任务中心',settings:'设置'}
const state={prefs:defaults(),tasks:null,settings:null,setting:'general',probes:{},providerPaths:{},busy:new Set()}
const generalKey='local-bridge-desktop-general-v1'
let general=generalPreferences(null)
try{general=generalPreferences(JSON.parse(localStorage.getItem(generalKey)))}catch{}
document.documentElement.dataset.theme=general.theme

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
  local:['本机',''],
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
  if(page==='settings'){void loadSettings();if(state.setting==='agent')void loadPreferences()}
}

const providerDetails={
  cloudfile:{name:'CloudFile',placeholder:'/absolute/path/to/cloudfile-native-host',
    args:'--bridge-provider',help:'安装支持 Bridge Provider 的 Agent2，使用原安装与原数据目录。普通 Native Host 不一定支持此模式。',
    ownership:'账号、服务器、文件目录、Guard 与 Journal 继续在原 CloudFile Agent2 中管理。'},
  wechat:{name:'微信',placeholder:'/absolute/path/to/wechat-provider',args:'serve',
    help:'安装独立 WeChat Provider 后，选择其虚拟环境中的 wechat-provider 可执行文件。',
    ownership:'账号与索引由 WeChat Provider 管理。Mac 实时读写尚未验收；本页检查只发现 MCP 工具，不操作微信。'}
}
function renderProvider(id){
  const target=el('provider-'+id),detail=providerDetails[id]
  target.replaceChildren()
  if(!state.settings){target.append(node('p','连接状态尚未读取，请在通用设置中刷新。'));return}
  const registered=state.settings.providers.some(p=>p.id===id)
  const probe=state.probes[id],status=connectionState(registered,probe)
  const panel=node('article',null,'panel settings-panel')
  const header=node('div',null,'panel-title'),badge=node('span',status.label,'pill '+status.kind)
  header.append(node('h3',detail.name+' Provider'),badge)
  panel.append(header,node('p',status.detail,'provider-description'))
  if(!registered){
    panel.append(node('p',detail.help,'provider-description'))
    const form=document.createElement('form'),label=node('label',null,'field')
    label.append(node('span','可执行文件绝对路径'))
    const input=document.createElement('input');input.type='text';input.required=true
    input.placeholder=detail.placeholder;input.autocomplete='off'
    input.value=state.providerPaths[id]||''
    input.addEventListener('input',()=>{state.providerPaths[id]=input.value})
    input.setAttribute('aria-label',detail.name+' Provider 可执行文件绝对路径')
    const button=node('button','载入 Provider','button primary');button.type='submit'
    button.disabled=state.busy.has(id)
    label.append(input,node('small','启动参数固定为 '+detail.args+'，不执行 Shell 命令。'))
    const actions=node('div',null,'actions');actions.append(button);form.append(label,actions)
    form.addEventListener('submit',event=>{
      event.preventDefault()
      void providerAction(id,()=>call('desktop_provider_add',{id,executable:input.value.trim()}),'已载入 Provider。可以检查连接。')
    })
    panel.append(form)
  }else{
    panel.append(node('p','连接参数：'+id+' · '+detail.args,'provider-description'))
    const actions=node('div',null,'actions')
    const remove=node('button','移除连接','button secondary');remove.type='button'
    remove.disabled=state.busy.has(id)
    remove.addEventListener('click',()=>void providerAction(id,
      ()=>call('desktop_provider_remove',{id}),'已移除 Bridge 连接。原程序及数据保留。'))
    const check=node('button',state.busy.has(id)?'正在处理…':'检查连接','button primary');check.type='button'
    check.disabled=state.busy.has(id)
    check.addEventListener('click',()=>void probeProvider(id))
    actions.append(remove,check);panel.append(actions)
    if(probe?.error)panel.append(node('p',probe.error,'muted-banner'))
    if(Array.isArray(probe?.tools)){
      const section=node('div',null,'auxiliary')
      section.append(node('h3','可用工具（'+probe.tools.length+'）'))
      const list=node('div',null,'source-strip')
      for(const tool of probe.tools)list.append(node('span',tool,'source-tag'))
      section.append(list,node('p','当前 Provider 未提供设置描述与保存接口；专属配置请在原程序中管理。'))
      panel.append(section)
    }
  }
  target.append(panel,node('p',detail.ownership,'footnote'))
}
async function loadSettings(){
  try{
    state.settings=null
    state.probes={}
    const data=await call('desktop_settings')
    if(!Array.isArray(data.providers))throw new Error('Provider 注册列表不可用')
    state.settings=data
    el('general-environment').replaceChildren(
      line('本机数据目录',data.data_root,'local'),
      line('Chrome Native Host','com.eaopen.browser_bridge',
        data.host?.state==='not_installed'?'not_installed':'unverified'),
      line('MCP Provider',data.providers.length+' 个已载入；检查连接后才能确认响应','local'))
  }catch(error){notice('设置读取失败：'+error)}
  finally{for(const id of Object.keys(providerDetails))renderProvider(id)}
}
async function providerAction(id,action,message){
  if(state.busy.has(id))return
  state.busy.add(id);renderProvider(id)
  try{
    await action();delete state.probes[id]
    await loadSettings();notice(message,true)
  }catch(error){notice('连接设置失败：'+error)}
  finally{state.busy.delete(id);renderProvider(id)}
}
async function probeProvider(id){
  if(state.busy.has(id))return
  state.busy.add(id);renderProvider(id)
  try{
    const result=await call('desktop_provider_tools',{id})
    if(!Array.isArray(result.tools))throw new Error('Provider 未返回工具列表')
    state.probes[id]={tools:result.tools}
  }catch(error){state.probes[id]={error:String(error)}}
  finally{state.busy.delete(id);renderProvider(id)}
}
function setSetting(name){
  if(!SETTINGS.includes(name))return
  state.setting=name;notice('')
  for(const key of SETTINGS)el('settings-'+key).hidden=key!==name
  for(const button of document.querySelectorAll('[data-setting]')){
    const selected=button.dataset.setting===name
    button.classList.toggle('selected',selected)
    if(selected)button.setAttribute('aria-current','page')
    else button.removeAttribute('aria-current')
  }
  if(name==='agent')void loadPreferences()
}
for(const button of document.querySelectorAll('[data-setting]'))
  button.addEventListener('click',()=>setSetting(button.dataset.setting))
el('general-theme').value=general.theme
el('general-start-page').value=general.startPage
el('general-form').addEventListener('submit',event=>{
  event.preventDefault()
  try{
    const next=generalPreferences({theme:el('general-theme').value,startPage:el('general-start-page').value})
    localStorage.setItem(generalKey,JSON.stringify(next))
    general=next;document.documentElement.dataset.theme=general.theme
    notice('通用设置已保存。',true)
  }catch(error){notice('保存失败：'+error)}
})
el('refresh-settings').addEventListener('click',()=>{state.probes={};void loadSettings()})
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
setPage(general.startPage)
