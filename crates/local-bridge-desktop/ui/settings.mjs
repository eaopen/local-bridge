export const SETTINGS = ['general','agent','cloudfile','wechat']
export function generalDefaults(){return {theme:'system',startPage:'overview'}}
export function generalPreferences(value){
  const result=generalDefaults()
  if(['system','light','dark'].includes(value?.theme))result.theme=value.theme
  if(['overview','tasks','settings'].includes(value?.startPage))result.startPage=value.startPage
  return result
}
export function connectionState(registered,probe){
  if(!registered)return {label:'未载入',kind:'',detail:'先安装 Provider，再填写它的可执行文件路径。'}
  if(probe?.error)return {label:'连接失败',kind:'bad',detail:'已注册，但工具发现失败。请检查安装与启动参数。'}
  if(Array.isArray(probe?.tools))return {label:'已连接',kind:'ok',detail:'MCP 工具发现成功；账号登录和业务能力仍由 Provider 核验。'}
  return {label:'已载入 · 待检查',kind:'warn',detail:'已注册到本机。点击检查连接，读取实际支持的工具。'}
}
