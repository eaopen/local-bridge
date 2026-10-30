// Pure UI helpers. Rust AgentPreferences::validate remains the authority.
export const AGENTS = ['workbuddy','codex','pi']
export const AGENT_LABELS = {workbuddy:'WorkBuddy',codex:'Codex',pi:'Pi'}
export const THINK_OPTIONS = {
  auto:['auto','off','low','medium','high','xhigh'],
  workbuddy:['auto','low','medium','high','xhigh','max'],
  codex:['auto','low','medium','high','xhigh'],
  pi:['auto','off','low','medium','high','xhigh']
}
export function defaults(){
  return {schema_version:1,agent:'auto',agent_mode:'prefer',model:'auto',
    think_mode:'auto',priority:[...AGENTS]}
}
export function shiftPriority(list, agent, direction){
  if(!Array.isArray(list)||!AGENTS.includes(agent)||![1,-1].includes(direction))
    throw new Error('invalid_priority')
  const out=[...list]
  const i=out.indexOf(agent), j=i+direction
  if(i<0||j<0||j>=out.length)return out
  ;[out[i],out[j]]=[out[j],out[i]]
  return out
}
export function normalizePreferenceDraft(value){
  if(!value||typeof value!=='object'||Array.isArray(value))
    throw new Error('invalid_preferences')
  const agent=value.agent
  if(!['auto',...AGENTS].includes(agent)||!['prefer','require'].includes(value.agent_mode))
    throw new Error('invalid_agent_or_mode')
  if(agent==='auto'&&value.agent_mode==='require')throw new Error('auto_cannot_require')
  const model=String(value.model||'').trim()
  if(model.length<1||model.length>128||! /^[A-Za-z0-9_./:-]+$/.test(model)||
     model.includes('://')||model.includes('..')||model.startsWith('/'))
    throw new Error('invalid_model')
  if(!THINK_OPTIONS[agent].includes(value.think_mode))
    throw new Error('unsupported_think_mode')
  if(agent==='workbuddy'&&model.includes('/'))
    throw new Error('invalid_workbuddy_model')
  const priority=value.priority
  if(!Array.isArray(priority)||priority.length!==AGENTS.length||
     AGENTS.some(v=>priority.filter(x=>x===v).length!==1))
    throw new Error('invalid_priority')
  return {schema_version:1,agent,agent_mode:value.agent_mode,
    model,think_mode:value.think_mode,priority:[...priority]}
}
