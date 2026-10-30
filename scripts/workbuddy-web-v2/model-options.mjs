// Resolve explicit options against the live account catalog before any prompt is sent.
// Never substitute another model or rewrite the account's model capabilities.
export function modelOptions(config,model='auto',think='auto'){
 const ids=config?.agents?.find(agent=>agent.name==='cli')?.models;
 const definition=config?.models?.find(item=>item.id===model);
 if(typeof model!=='string'||!/^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(model)||
    !ids?.includes(model)||!definition||definition.disabled===true)throw Error('model_unavailable');
 const args=model==='auto'?[]:['--model',model];
 if(think!=='auto'){
  const reasoning=definition.reasoning;
  const supported=reasoning?.supportedEfforts??[reasoning?.effort??reasoning?.defaultEffort];
  if(!['low','medium','high','xhigh','max'].includes(think)||
     !Array.isArray(supported)||!supported.includes(think))throw Error('effort_unavailable');
  args.push('--effort',think);
 }
 return args;
}
