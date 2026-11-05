import {useEffect,useMemo,useState} from 'preact/hooks';
import {consultarAuditoria,lanzarAuditoria} from '../stores/webs';
import {authHeaders} from '../api/client';
import type {components} from '../api/schema';
import {categoriaEs, errorAuditoriaEs, estadoAuditoriaEs, etiquetaMetricaEs, gravedadEs, traducirHallazgo} from '../diagnostico-es';

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

function InformeOriginal({report}:{report:Report}){
 const [src,setSrc]=useState(''),[error,setError]=useState('');
 useEffect(()=>{
  let live=true,objectUrl='';setSrc('');setError('');
  fetch(`/api/auditorias-tecnicas/${encodeURIComponent(report.id)}/archivo/html/preview`,{headers:authHeaders(),credentials:'same-origin'})
   .then(async response=>{if(!response.ok)throw new Error(response.status===404?'El HTML original ya no está disponible.':`No se pudo cargar el informe (${response.status}).`);return response.blob();})
   .then(blob=>{objectUrl=URL.createObjectURL(new Blob([blob],{type:'text/html'}));if(live)setSrc(objectUrl);else URL.revokeObjectURL(objectUrl);})
   .catch(e=>{if(live)setError(errorAuditoriaEs(e));});
  return()=>{live=false;if(objectUrl)URL.revokeObjectURL(objectUrl);};
 },[report.id]);
 const descargar=async()=>{
  try{const response=await fetch(`/api/auditorias-tecnicas/${encodeURIComponent(report.id)}/archivo/html`,{headers:authHeaders(),credentials:'same-origin'});if(!response.ok)throw new Error('El HTML original no está disponible.');const blob=await response.blob();const url=URL.createObjectURL(new Blob([blob],{type:'text/html'}));const a=document.createElement('a');a.href=url;a.download=`${report.motor}-${report.dispositivo}-${report.id}.html`;a.click();URL.revokeObjectURL(url);}
  catch(e){setError(errorAuditoriaEs(e));}
 };
 return <section class="informe-html">
  <header class="informe-html-cabecera"><div><strong>Informe original · {report.motor==='lighthouse'?'Lighthouse':'SiteOne'}</strong><small class="muted">Puede conservar términos técnicos del motor · versión {report.version||'—'} · {new Date(report.fecha).toLocaleString('es-ES')}</small></div><button class="secundario" onClick={()=>void descargar()}>Descargar informe original</button></header>
  {error&&<p class="error">{error}</p>}
  {src?<iframe title={`Informe HTML ${report.motor} ${report.dispositivo}`} src={src} sandbox="allow-scripts" referrerPolicy="no-referrer" />:<div class="informe-cargando muted">{error?'No se pudo abrir el informe.':'Cargando HTML original…'}</div>}
 </section>;
}

function InformeEspanol({report}:{report:Report}){
 const [originalOpen,setOriginalOpen]=useState(false);
 const result=report.resultado as any;
 const scores=Object.entries(result?.scores??{}) as [string,unknown][];
 const metrics=(result?.metrics??[]) as any[];
 const issues=(result?.issues??[]) as any[];
 const coverage=result?.coverage??{};
 const scale=Number(result?.score_scale??100);
 return <section class="informe-espanol">
  <header class="informe-espanol-cabecera">
   <div><span class="informe-eyebrow">{report.motor==='lighthouse'?'Lighthouse':'SiteOne'} · {report.dispositivo==='mobile'?'Móvil':'Escritorio'}</span><h3>Resultados de la revisión</h3>
    <p>{estadoAuditoriaEs(report.estado)} · {new Date(report.fecha).toLocaleString('es-ES')} · versión {report.version||'—'}</p></div>
   <span class={`badge estado-audit estado-audit-${report.estado}`}>{estadoAuditoriaEs(report.estado)}</span>
  </header>
  {scores.length>0&&<section class="informe-metricas" aria-label="Puntuaciones">
   {scores.map(([key,value])=>{const numeric=typeof value==='number'?value:null;const normalized=numeric===null?0:scale===10?numeric*10:numeric;const quality=normalized>=80?'buena':normalized>=50?'media':'baja';return <article class={`informe-puntuacion puntuacion-${quality}`} key={key}>
     <span>{etiquetaMetricaEs(key)}</span><strong>{numeric===null?'—':numeric}{numeric!==null?`/${scale}`:''}</strong>
     {numeric!==null&&<div class="informe-barra"><i style={{width:`${Math.max(0,Math.min(100,normalized))}%`}}/></div>}
   </article>})}
  </section>}
  {metrics.length>0&&<section class="informe-metricas informe-vitales" aria-label="Tiempos de carga"><h4>Experiencia de carga</h4>{metrics.map((metric:any)=><article key={metric.code}>
    <span>{etiquetaMetricaEs(metric.code,metric.label)}</span><strong>{metric.display||'Sin medición'}</strong>
   </article>)}</section>}
  {(coverage.html_pages!==undefined||coverage.visited!==undefined)&&<section class="informe-cobertura"><h4>Alcance revisado</h4><div>
   <span><strong>{coverage.html_pages??coverage.visited??0}</strong> páginas analizadas</span>
   {coverage.rendered_pages!==undefined&&<span><strong>{coverage.rendered_pages}</strong> páginas con JavaScript</span>}
   {coverage.blocked>0&&<span><strong>{coverage.blocked}</strong> no accesibles al rastreador</span>}
   {coverage.omitted>0&&<span><strong>{coverage.omitted}</strong> fuera del límite de rastreo</span>}
  </div></section>}
  <section class="informe-hallazgos"><header><div><h4>Aspectos a revisar</h4><p>{issues.length?`${issues.length} observaciones técnicas; contrástalas antes de decidir cambios.`:'No hay hallazgos en los datos disponibles.'}</p></div></header>
   {issues.slice(0,40).map((issue:any,index:number)=>{const translated=traducirHallazgo(issue);return <article class="informe-hallazgo" key={`${issue.code||index}-${index}`}>
    <div class="informe-hallazgo-top"><span class="badge">{categoriaEs(issue.category)}</span><span class={`diagnostico-gravedad gravedad-${issue.severity||'info'}`}>{gravedadEs(issue.severity)}</span></div>
    <h5>{translated.titulo}</h5><p>{translated.explicacion}</p><p class="informe-siguiente"><strong>Siguiente paso:</strong> {translated.siguientePaso}</p>
    {issue.url&&<small class="muted informe-url">Página: <a href={issue.url} target="_blank" rel="noreferrer">{issue.url}</a></small>}
    {(issue.title||issue.details||issue.recommendation||issue.code)&&<details class="informe-detalle-original"><summary>Ver detalle técnico original</summary><p><strong>{translated.original}</strong></p>{issue.details&&<p>{issue.details}</p>}{issue.recommendation&&<p>{issue.recommendation}</p>}{issue.code&&<small>Código: {issue.code}</small>}</details>}
   </article>})}
   {issues.length>40&&<p class="muted">Se muestran los primeros 40 hallazgos. El informe original conserva el detalle completo.</p>}
  </section>
  {(result?.warnings?.length||result?.runtime_error)&&<p class="informe-aviso" role="note">El motor indicó avisos o límites durante la medición. Revisa el informe original antes de interpretar la cobertura.</p>}
  <details class="informe-original-detalle" onToggle={event=>setOriginalOpen((event.currentTarget as HTMLDetailsElement).open)}><summary>Consultar informe HTML original de {report.motor==='lighthouse'?'Lighthouse':'SiteOne'}</summary>{originalOpen&&<InformeOriginal report={report}/>}</details>
 </section>;
}

export function AuditoriaTecnica({id,web,mode,onAvailabilityChange}:{id:number;web?:string|null;mode:Mode;onAvailabilityChange?:(motors:Engine[])=>void}){
 const [data,setData]=useState<Estado|null>(null),[deviceByEngine,setDeviceByEngine]=useState<Record<Engine,string>>({lighthouse:'mobile',siteone:'mobile'}),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 useEffect(()=>{let live=true,timer:ReturnType<typeof setTimeout>;setData(null);setDeviceByEngine({lighthouse:'mobile',siteone:'mobile'});setError('');
  const load=async()=>{try{const d=await consultarAuditoria(id);if(live){setData(d);if(['pendiente','en_curso'].includes(d.job?.estado??''))timer=setTimeout(load,5000);}}catch(e){if(live)setError(errorAuditoriaEs(e));}};void load();return()=>{live=false;clearTimeout(timer);};
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
  {activeReport?<InformeEspanol key={activeReport.id} report={activeReport}/>:<div class="informe-cargando muted">Todavía no hay un informe HTML para este dispositivo.</div>}
 </section>;
 return <section class="auditoria-tecnica">
  <div class="auditoria-estados" aria-label="Estado de auditorías">{motores.flatMap(([m])=>dispositivos.map(([d])=><EstadoFase key={`${m}-${d}`} motor={m} device={d} data={data} url={web}/>))}</div>
  {data&&!data.worker_disponible&&<p class="muted">Worker no disponible en este momento. Los trabajos quedan en cola.</p>}
  <div class="auditoria-acciones"><button disabled={busy||!web||enCurso} onClick={async()=>{setBusy(true);setError('');try{setData(await lanzarAuditoria(id));}catch(e){setError(errorAuditoriaEs(e));}finally{setBusy(false);}}}>{enCurso?'Auditoría en cola / en curso':busy?'Lanzando…':available.length?'Repetir auditorías':'Lanzar auditorías'}</button>{data?.job&&<span class="muted" role="status">{data.job.hechos}/{data.job.total} fases · {({pendiente:'En cola',en_curso:'Auditando',terminado:'Finalizada',error:'Finalizada con incidencias'} as Record<string,string>)[data.job.estado]}{data.job.error&&<span class="error"> · {errorAuditoriaEs(data.job.error)}</span>}</span>}</div>
  {!web&&<p class="muted">Confirma una web propia para poder auditarla.</p>}{error&&<p class="error">{error}</p>}
  <p class="muted">Las webs propias confirmadas se revisan automáticamente cada 24 horas. Los resultados presentados por Radar están en español; los informes originales se conservan como referencia técnica.</p>
 </section>;
}

export function AuditoriaInformeAparte({id,engine,initialDevice}:{id:number;engine:Engine;initialDevice:'mobile'|'desktop'}){
 const [data,setData]=useState<Estado|null>(null),[device,setDevice]=useState<'mobile'|'desktop'>(initialDevice),[loading,setLoading]=useState(true),[error,setError]=useState('');
 useEffect(()=>{let live=true;consultarAuditoria(id).then(result=>{if(live){setData(result);setLoading(false)}}).catch(problem=>{if(live){setError(errorAuditoriaEs(problem));setLoading(false)}});return()=>{live=false}},[id]);
 const report=useMemo(()=>data?.informes.filter(item=>item.motor===engine&&item.dispositivo===device).sort((a,b)=>b.fecha.localeCompare(a.fecha))[0],[data,engine,device]);
 useEffect(()=>{if(loading||!data||device!=='mobile')return;const mobile=data.informes.some(item=>item.motor===engine&&item.dispositivo==='mobile');const desktop=data.informes.some(item=>item.motor===engine&&item.dispositivo==='desktop');if(!mobile&&desktop){setDevice('desktop');history.replaceState(null,'',`/auditoria-tecnica/${id}/${engine}/desktop`)}},[loading,data,device,id,engine]);
 const changeDevice=(next:'mobile'|'desktop')=>{setDevice(next);history.replaceState(null,'',`/auditoria-tecnica/${id}/${engine}/${next}`)};
 const back=()=>{if(history.length>1)history.back();else window.location.assign('/')};
 return <section class="informe-aparte">
  <header class="informe-aparte-cabecera">
   <div><span class="informe-eyebrow">{engine==='lighthouse'?'Lighthouse':'SiteOne'}</span><h1>Informe técnico</h1></div>
   <div class="auditoria-dispositivos" role="tablist" aria-label="Dispositivo del informe">{dispositivos.map(([value,label])=><button type="button" key={value} role="tab" aria-selected={device===value} class={device===value?'':'secundario'} onClick={()=>changeDevice(value)}>{label}</button>)}</div>
  </header>
  {loading?<div class="informe-cargando muted">Cargando informe…</div>:error?<p class="error">{error}</p>:report?<InformeEspanol key={report.id} report={report}/>:<p class="diagnostico-vacio muted">No hay un informe de {engine==='lighthouse'?'Lighthouse':'SiteOne'} para {device==='mobile'?'móvil':'escritorio'}.</p>}
  <button type="button" class="informe-aparte-volver secundario" onClick={back}>← Volver a Radar Navarra</button>
 </section>;
}
