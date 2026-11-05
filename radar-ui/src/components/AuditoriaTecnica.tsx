import {useEffect,useMemo,useState} from 'preact/hooks';
import {consultarAuditoria,lanzarAuditoria} from '../stores/webs';
import type {components} from '../api/schema';
import {categoriaEs, errorAuditoriaEs, estadoAuditoriaEs, etiquetaMetricaEs, gravedadEs, traducirHallazgo} from '../diagnostico-es';
import {Desplegable} from './Desplegable';

type Estado=components['schemas']['EstadoAuditoriaTecnica'];
type Report=components['schemas']['AuditoriaTecnica'];
export type Engine='lighthouse'|'siteone';
export type Device='mobile'|'desktop';
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

function InformeEspanol({report}:{report:Report}){
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
   <div class="informe-espanol-acciones"><span class={`badge estado-audit estado-audit-${report.estado}`}>{estadoAuditoriaEs(report.estado)}</span><a class="informe-original-enlace" href={`/api/auditorias-tecnicas/${encodeURIComponent(report.id)}/archivo/html/preview`} target="_blank" rel="noopener noreferrer">Ver informe original</a></div>
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
    {(issue.title||issue.details||issue.recommendation||issue.code)&&<Desplegable className="informe-detalle-original" titulo="Ver detalle técnico original"><p><strong>{translated.original}</strong></p>{issue.details&&<p>{issue.details}</p>}{issue.recommendation&&<p>{issue.recommendation}</p>}{issue.code&&<small>Código: {issue.code}</small>}</Desplegable>}
   </article>})}
   {issues.length>40&&<p class="muted">Se muestran los primeros 40 hallazgos. El informe original conserva el detalle completo.</p>}
  </section>
  {(result?.warnings?.length||result?.runtime_error)&&<p class="informe-aviso" role="note">El motor indicó avisos o límites durante la medición. Revisa el informe original antes de interpretar la cobertura.</p>}
 </section>;
}

export function AuditoriaTecnica({id,web,mode,onAvailabilityChange,deviceByEngine:controlledDevices,onDeviceChange}:{id:number;web?:string|null;mode:Mode;onAvailabilityChange?:(motors:Engine[])=>void;deviceByEngine?:Record<Engine,Device>;onDeviceChange?:(engine:Engine,device:Device)=>void}){
 const [data,setData]=useState<Estado|null>(null),[internalDevices,setInternalDevices]=useState<Record<Engine,Device>>({lighthouse:'mobile',siteone:'mobile'}),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 const deviceByEngine=controlledDevices??internalDevices;
 const setDevice=(engine:Engine,device:Device)=>controlledDevices&&onDeviceChange?onDeviceChange(engine,device):setInternalDevices(old=>({...old,[engine]:device}));
 useEffect(()=>{let live=true,timer:ReturnType<typeof setTimeout>;setData(null);if(!controlledDevices)setInternalDevices({lighthouse:'mobile',siteone:'mobile'});setError('');
  const load=async()=>{try{const d=await consultarAuditoria(id);if(live){setData(d);if(['pendiente','en_curso'].includes(d.job?.estado??''))timer=setTimeout(load,5000);}}catch(e){if(live)setError(errorAuditoriaEs(e));}};void load();return()=>{live=false;clearTimeout(timer);};
 },[id,web,busy]);
 const latest=useMemo(()=>{const out:Record<string,Report>={};for(const r of data?.informes??[]){if(web&&r.url!==web)continue;if(!r.archivos.includes('html'))continue;const key=`${r.motor}:${r.dispositivo}`;if(!out[key]||r.fecha>out[key].fecha)out[key]=r;}return out;},[data,web]);
 const available=useMemo(()=>motores.filter(([key])=>Object.keys(latest).some(k=>k.startsWith(`${key}:`))).map(([key])=>key),[latest]);
 const availableKey=available.join(',');
 useEffect(()=>{onAvailabilityChange?.(available);},[availableKey,id,onAvailabilityChange]);
 useEffect(()=>{for(const engine of ['lighthouse','siteone'] as Engine[]){if(!latest[`${engine}:mobile`]&&latest[`${engine}:desktop`]&&deviceByEngine[engine]!=='desktop')setDevice(engine,'desktop');}},[latest]);
 const selectedEngine=mode==='lighthouse'||mode==='siteone'?mode:null;
 const device=selectedEngine?deviceByEngine[selectedEngine]:'mobile';
 const activeReport=selectedEngine?latest[`${selectedEngine}:${device}`]:undefined;
 const enCurso=['pendiente','en_curso'].includes(data?.job?.estado??'');
 if(mode===null)return null;
 if(mode!=='summary')return <section class="auditoria-informe-tab">
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
