export function agentHealthView(agent,check,checking,now=Date.now(),queued=false){
  if(!agent.installed)return {label:'未安装',kind:'',detail:'请先安装原 Agent 客户端或 CLI。'}
  if(!agent.run_supported)return {label:'平台未支持',kind:'warn',detail:'已安装，但当前平台的 Bridge 适配器尚不支持运行。'}
  if(checking)return {label:'检查中',kind:'warn',detail:'正在验证当前账号和固定短任务，请等待结果。'}
  if(queued)return {label:'等待检查',kind:'',detail:'已加入检查队列，当前检查完成后继续。'}
  if(!check)return {label:'待检查',kind:'warn',detail:'已安装；认证与调用能力尚未确认。'}
  if(!Number.isFinite(check.checked_at_unix_ms)||now-check.checked_at_unix_ms>300000)
    return {label:'需重新检查',kind:'warn',detail:'先前的检查结果已过期，请重新确认当前状态。'}
  const passed=check.ready===true&&check.authenticated===true&&check.api_ready===true&&check.run_completed===true
  return {label:passed?'可用':check.state==='not_authenticated'?'未登录':'未通过',kind:passed?'ok':'bad',detail:check.reason||'可用性尚未确认。'}
}

// One model check at a time. Repeated clicks share the same pending result.
export function createAgentCheckQueue(run,onChange=()=>{}){
  const jobs=[],pending=new Map()
  let active=null,draining=false
  const changed=()=>onChange({active,pending:jobs.map(job=>job.agent)})
  async function drain(){
    if(draining)return
    draining=true
    while(jobs.length){
      const job=jobs.shift();active=job.agent;changed()
      try{job.resolve(await run(job.agent))}catch(error){job.reject(error)}
      finally{pending.delete(job.agent);active=null;changed()}
    }
    draining=false
  }
  return {
    enqueue(agent){
      if(pending.has(agent))return pending.get(agent)
      let resolve,reject
      const promise=new Promise((yes,no)=>{resolve=yes;reject=no})
      pending.set(agent,promise);jobs.push({agent,resolve,reject});changed()
      void drain()
      return promise
    }
  }
}
