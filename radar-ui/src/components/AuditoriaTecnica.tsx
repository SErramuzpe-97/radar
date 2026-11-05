import {useEffect,useMemo,useState} from 'preact/hooks';
import {consultarAuditoria,lanzarAuditoria} from '../stores/webs';
import {authHeaders} from '../api/client';
import type {components} from '../api/schema';

type Estado=components['schemas']['EstadoAuditoriaTecnica'];
type Report=components['schemas']['AuditoriaTecnica'];
type Engine='lighthouse'|'siteone';
type Mode='summary'|Engine|null;
const motores:[Engine,string][]=[['lighthouse','Lighthouse'],['siteone','SiteOne']];
const dispositivos=[['mobile','Móvil'],['desktop','Escritorio']] as const;
const etiquetas:Record<string,string>={realizada:'Realizada',en_curso:'En curso',pendiente:'Pendiente',no_lanzada:'No lanzada',error:'Error',no_disponible:'No disponible'};

function EstadoFase({motor,device,data,url}:{motor:Engine;device:string;data:Estado|null;url:string|null|undefined}){
 const reports=(data?.informes??[]).filter(r=>r.motor===motor&&r.dispositivo===device&&(!url||r.url===url)).sort((a,b)=>b.fecha.localeCompare(a.fecha));
 const r=reports[0];let status='no_lanzada';
 if(r)status=['completo','parcial'].includes(r.estado)?'realizada':r.estado==='pendiente'||r.estado==='en_curso'?r.estado:r.estado==='error'?'error':'no_disponible';
 else if(data?.job&&(!url||data.job.params&&typeof data.job.params==='object'&&(data.job.params as Record<string,unknown>).url===url)){
  const summary=data.job.resumen&&typeof data.job.resumen==='object'?data.job.resumen as Record<string,unknown>:{};
  const active=summary.motor_actual===motor&&summary.dispositivo_actual===device;
  if(data.job.estado==='pendiente')status='pendiente';else if(data.job.estado==='en_curso')status=active?'en_curso':'pendiente';else if(data.job.estado==='error'&&active)status='error';
 }
 return <span class={`badge estado-audit estado-audit-${status}`} title={`${motor==='lighthouse'?'Lighthouse':'SiteOne'} · ${device==='mobile'?'móvil':'escritorio'}`}>
  {motor==='lighthouse'?'Lighthouse':'SiteOne'} · {device==='mobile'?'Móvil':'Escritorio'}: {etiquetas[status]??status}
 </span>;
}

function InformeHtml({report}:{report:Report}){
 const [src,setSrc]=useState(''),[error,setError]=useState('');
 useEffect(()=>{
  let live=true,objectUrl='';setSrc('');setError('');
  fetch(`/api/auditorias-tecnicas/${encodeURIComponent(report.id)}/archivo/html/preview`,{headers:authHeaders(),credentials:'same-origin'})
   .then(async response=>{if(!response.ok)throw new Error(response.status===404?'El HTML original ya no está disponible.':`No se pudo cargar el informe (${response.status}).`);return response.blob();})
   .then(blob=>{objectUrl=URL.createObjectURL(new Blob([blob],{type:'text/html'}));if(live)setSrc(objectUrl);else URL.revokeObjectURL(objectUrl);})
   .catch(e=>{if(live)setError(String(e));});
  return()=>{live=false;if(objectUrl)URL.revokeObjectURL(objectUrl);};
 },[report.id]);
 const descargar=async()=>{
  try{const response=await fetch(`/api/auditorias-tecnicas/${encodeURIComponent(report.id)}/archivo/html`,{headers:authHeaders(),credentials:'same-origin'});if(!response.ok)throw new Error('El HTML original no está disponible.');const blob=await response.blob();const url=URL.createObjectURL(new Blob([blob],{type:'text/html'}));const a=document.createElement('a');a.href=url;a.download=`${report.motor}-${report.dispositivo}-${report.id}.html`;a.click();URL.revokeObjectURL(url);}
  catch(e){setError(String(e));}
 };
 return <section class="informe-html">
  <header class="informe-html-cabecera"><div><strong>{report.motor==='lighthouse'?'Lighthouse':'SiteOne'} · {report.dispositivo==='mobile'?'Móvil':'Escritorio'}</strong><small class="muted">Informe original · versión {report.version||'—'} · {new Date(report.fecha).toLocaleString('es-ES')}</small></div><button class="secundario" onClick={()=>void descargar()}>Descargar HTML</button></header>
  {error&&<p class="error">{error}</p>}
  {src?<iframe title={`Informe HTML ${report.motor} ${report.dispositivo}`} src={src} sandbox="allow-scripts" referrerPolicy="no-referrer" />:<div class="informe-cargando muted">{error?'No se pudo abrir el informe.':'Cargando HTML original…'}</div>}
 </section>;
}

export function AuditoriaTecnica({id,web,mode,onAvailabilityChange}:{id:number;web?:string|null;mode:Mode;onAvailabilityChange?:(motors:Engine[])=>void}){
 const [data,setData]=useState<Estado|null>(null),[deviceByEngine,setDeviceByEngine]=useState<Record<Engine,string>>({lighthouse:'mobile',siteone:'mobile'}),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 useEffect(()=>{let live=true,timer:ReturnType<typeof setTimeout>;setData(null);setDeviceByEngine({lighthouse:'mobile',siteone:'mobile'});setError('');
  const load=async()=>{try{const d=await consultarAuditoria(id);if(live){setData(d);if(['pendiente','en_curso'].includes(d.job?.estado??''))timer=setTimeout(load,5000);}}catch(e){if(live)setError(String(e));}};void load();return()=>{live=false;clearTimeout(timer);};
 },[id,web,busy]);
 const latest=useMemo(()=>{const out:Record<string,Report>={};for(const r of data?.informes??[]){if(web&&r.url!==web)continue;if(!r.archivos.includes('html'))continue;const key=`${r.motor}:${r.dispositivo}`;if(!out[key]||r.fecha>out[key].fecha)out[key]=r;}return out;},[data,web]);
 const available=useMemo(()=>motores.filter(([key])=>Object.keys(latest).some(k=>k.startsWith(`${key}:`))).map(([key])=>key),[latest]);
 const availableKey=available.join(',');
 useEffect(()=>{onAvailabilityChange?.(available);},[availableKey,id,onAvailabilityChange]);
 useEffect(()=>{for(const engine of ['lighthouse','siteone'] as Engine[]){if(!latest[`${engine}:mobile`]&&latest[`${engine}:desktop`])setDeviceByEngine(old=>old[engine]==='desktop'?old:{...old,[engine]:'desktop'});}},[latest]);
 const selectedEngine=mode==='lighthouse'||mode==='siteone'?mode:null;
 const device=selectedEngine?deviceByEngine[selectedEngine]:'mobile';
 const activeReport=selectedEngine?latest[`${selectedEngine}:${device}`]:undefined;
 const enCurso=['pendiente','en_curso'].includes(data?.job?.estado??'');
 if(mode===null)return null;
 if(mode!=='summary')return <section class="auditoria-informe-tab">
  <div class="auditoria-dispositivos" role="tablist" aria-label="Dispositivo auditado">{dispositivos.map(([d,label])=><button key={d} role="tab" aria-selected={device===d} class={device===d?'':'secundario'} onClick={()=>selectedEngine&&setDeviceByEngine(old=>({...old,[selectedEngine]:d}))}>{label}</button>)}</div>
  {selectedEngine==='siteone'&&<small class="muted auditoria-aviso-idioma">SiteOne no ofrece traducción de informes en esta versión; los informes del motor se mantienen originales.</small>}
  {activeReport?<InformeHtml key={activeReport.id} report={activeReport}/>:<div class="informe-cargando muted">No hay informe HTML para este dispositivo.</div>}
 </section>;
 return <section class="auditoria-tecnica">
  <div class="auditoria-estados" aria-label="Estado de auditorías">{motores.flatMap(([m])=>dispositivos.map(([d])=><EstadoFase key={`${m}-${d}`} motor={m} device={d} data={data} url={web}/>))}</div>
  {data&&!data.worker_disponible&&<p class="muted">Worker no disponible en este momento. Los trabajos quedan en cola.</p>}
  <div class="auditoria-acciones"><button disabled={busy||!web||enCurso} onClick={async()=>{setBusy(true);setError('');try{setData(await lanzarAuditoria(id));}catch(e){setError(String(e));}finally{setBusy(false);}}}>{enCurso?'Auditoría en cola / en curso':busy?'Lanzando…':available.length?'Repetir auditorías':'Lanzar auditorías'}</button>{data?.job&&<span class="muted" role="status">{data.job.hechos}/{data.job.total} fases · {({pendiente:'En cola',en_curso:'Auditando',terminado:'Finalizada',error:'Finalizada con incidencias'} as Record<string,string>)[data.job.estado]}{data.job.error&&<span class="error"> · {data.job.error}</span>}</span>}</div>
  {!web&&<p class="muted">Confirma una web propia para poder auditarla.</p>}{error&&<p class="error">{error}</p>}
  <p class="muted">Las webs propias confirmadas se auditan automáticamente cada 24 horas. Lighthouse genera el informe en español; SiteOne conserva el idioma original cuando no hay traducción disponible.</p>
 </section>;
}
