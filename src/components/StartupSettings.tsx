import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {errorMessage} from "../types";
export function StartupSettings({disabled,onSave}:{disabled:boolean;onSave:(enabled:boolean)=>Promise<boolean>}) {
  const [enabled,setEnabled]=useState<boolean|null>(null);
  const [pending,setPending]=useState(false);
  const [error,setError]=useState<string|null>(null);
  useEffect(()=>{
    let active=true;
    void invoke<boolean>("get_startup_enabled").then(value=>{
      if(typeof value!=="boolean") throw "Could not read Windows startup registration.";
      if(active)setEnabled(value);
    }).catch(error=>{if(active)setError(errorMessage(error));});
    return ()=>{active=false;};
  },[]);
  async function toggle(){
    if(enabled===null||pending||disabled)return;
    setPending(true);setError(null);
    try{setEnabled(await onSave(!enabled));}
    catch(error){
      setError(errorMessage(error));
      try{const actual=await invoke<boolean>("get_startup_enabled");setEnabled(typeof actual==="boolean"?actual:null);}catch{setEnabled(null);}
    }finally{setPending(false);}
  }
  return <div><label className="toggle"><input type="checkbox" checked={enabled??false} disabled={disabled||pending||enabled===null} onChange={()=>void toggle()}/> Start with Windows</label>
    <p className="muted">Starts in the tray at sign-in. The toggle reflects Windows registration.</p>
    {error?<p role="alert" className="error">{error}</p>:null}</div>;
}
