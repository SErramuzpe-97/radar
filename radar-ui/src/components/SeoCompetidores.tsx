import { useEffect, useState } from 'preact/hooks';
import { consultarComparacionesSeo, consultarComparacionSeo, iniciarComparacionSeo, sugerirCompetidoresSeo, type SeoCompetidor, type SeoComparacion } from '../stores/webs';

export function SeoCompetidores({id,onChange}:{id:number;onChange?:(analysis:SeoComparacion|null)=>void}){
  const [items,setItems]=useState<SeoComparacion[]>([]);
  const [current,setCurrent]=useState<SeoComparacion|null>(null);
  const [selected,setSelected]=useState<Record<string,boolean>>({});
  const [extraName,setExtraName]=useState('');const [extraUrl,setExtraUrl]=useState('');
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  const refresh=async()=>{const history=await consultarComparacionesSeo(id);setItems(history);const newest=history[0]??null;setCurrent(newest);onChange?.(newest);return newest;};
  useEffect(()=>{let active=true;consultarComparacionesSeo(id).then(history=>{if(active){setItems(history);const newest=history[0]??null;setCurrent(newest);onChange?.(newest);}}).catch(e=>{if(active)setError(e.message)});return()=>{active=false}},[id]);
  useEffect(()=>{if(!current?.job_id||!['pendiente','en_curso'].includes(current.estado))return;let live=true;const timer=setInterval(()=>{void consultarComparacionSeo(id,current.id).then(value=>{if(!live)return;setCurrent(value);onChange?.(value);setItems(prev=>[value,...prev.filter(x=>x.id!==value.id)]);}).catch(()=>{});},2500);return()=>{live=false;clearInterval(timer)}},[id,current?.id,current?.estado,current?.job_id]);
  const suggest=async()=>{setBusy(true);setError('');try{const value=await sugerirCompetidoresSeo(id);setCurrent(value);setItems(prev=>[value,...prev.filter(x=>x.id!==value.id)]);onChange?.(value);setSelected({});}catch(e:any){setError(e.message)}finally{setBusy(false)}};
  const toggle=(url:string)=>setSelected(current=>({...current,[url]:!current[url]}));
  const analyze=async()=>{if(!current)return;const chosen=current.sugerencias.filter(item=>selected[item.url]);if(extraName.trim()&&extraUrl.trim())chosen.push({negocio_id:null,nombre:extraName.trim(),municipio:null,url:extraUrl.trim(),puntuacion_ia:null,fuentes:[]});if(!chosen.length){setError('Selecciona al menos un competidor o añade un dominio.');return;}setBusy(true);setError('');try{await iniciarComparacionSeo(id,current.id,chosen);await refresh();setSelected({});setExtraName('');setExtraUrl('');}catch(e:any){setError(e.message)}finally{setBusy(false)}};
  const retry=async()=>{if(!current)return;setBusy(true);setError('');try{await iniciarComparacionSeo(id,current.id,current.seleccion);await refresh()}catch(e:any){setError(e.message)}finally{setBusy(false)}};
  const result=current?.resultado??{};const sites=Array.isArray(result.sitios)?result.sitios:[];
  return <section class="seo-competidores">
    <header><div><h3>Comparación SEO con competidores</h3><p class="muted">Se miden señales públicas de un máximo de cinco dominios. No incluye posiciones, tráfico ni backlinks.</p></div><button type="button" disabled={busy||!current?.url_propia} onClick={()=>void suggest()}>{busy?'Procesando…':'Buscar competidores'}</button></header>
    {error&&<p class="error">{error}</p>}
    {current&&<p class="muted">Estado: {current.estado.replaceAll('_',' ')} · actualizado {new Date(current.actualizado).toLocaleString('es-ES')}{current.error?` · ${current.error}`:''}</p>}
    {current?.estado==='seleccion'&&<>
      <p>Revisa las sugerencias antes de rastrear. La selección final siempre es manual.</p>
      <div class="seo-candidatos">{current.sugerencias.map(candidate=><label class="seo-candidato" key={candidate.url}><input type="checkbox" checked={Boolean(selected[candidate.url])} onChange={()=>toggle(candidate.url)} disabled={!selected[candidate.url]&&Object.values(selected).filter(Boolean).length>=5}/><span><strong>{candidate.nombre}</strong> · {candidate.municipio||'Localidad no indicada'}<small><a href={candidate.url} target="_blank" rel="noreferrer">{candidate.url}</a> · comparabilidad IA {candidate.puntuacion_ia===3?'alta':'parcial'} · validar identidad y actividad</small>{candidate.fuentes?.length>0&&<small>Fuentes de consulta: {(candidate.fuentes as any[]).map(x=>x.title||x.url).join(' · ')}</small>}</span></label>)}</div>
      <details><summary>Añadir otro competidor</summary><div class="seo-extra"><input aria-label="Nombre de competidor" placeholder="Nombre del negocio" value={extraName} onInput={e=>setExtraName(e.currentTarget.value)}/><input aria-label="URL del competidor" placeholder="https://ejemplo.es" value={extraUrl} onInput={e=>setExtraUrl(e.currentTarget.value)}/></div></details>
      <button type="button" disabled={busy||Object.values(selected).filter(Boolean).length+(extraName.trim()&&extraUrl.trim()?1:0)>5} onClick={()=>void analyze()}>{busy?'Iniciando…':'Analizar dominios seleccionados'}</button>
    </>}
    {current?.estado==='obsoleta'&&<p class="error">La ficha propia cambió. Esta comparación se conserva como histórica; genera una nueva para usar datos vigentes.</p>}
    {current?.estado==='error'&&current.seleccion.length>0&&<button type="button" disabled={busy} onClick={()=>void retry()}>{busy?'Reintentando…':'Reintentar comparación con la misma selección'}</button>}
    {current&&['terminado','en_curso','error'].includes(current.estado)&&<>
      {sites.map((site:any)=><details class="seo-sitio" key={site.key}><summary>{site.nombre} · {site.url} · {site.datos?.estado} ({site.datos?.cobertura?.visitadas??0} páginas medidas)</summary><p>{JSON.stringify(site.datos?.metricas??site.datos?.error??{})}</p>{site.datos?.paginas?.map((page:any)=><p key={page.url}><a href={page.url} target="_blank" rel="noreferrer">{page.url}</a> · título: {page.titulo||'ausente'} · H1: {page.h1?.join(', ')||'ausente'} · HTTP {page.status}</p>)}</details>)}
      {result.comparativas?.length>0&&<table><thead><tr><th>Señal</th><th>Web propia</th><th>Competidor</th></tr></thead><tbody>{result.comparativas.flatMap((comparison:any)=>comparison.metricas.map((metric:any)=><tr key={`${comparison.url}-${metric.nombre}`}><td>{comparison.competidor}: {metric.nombre}</td><td>{metric.propia??'—'}</td><td>{metric.competidor??'—'}</td></tr>))}</tbody></table>}
      {result.informe&&<article><h4>Lectura asistida por IA</h4><p>{result.informe.texto}</p><small>{new Date(result.informe.generado).toLocaleString('es-ES')} · {result.informe.modelo} · recomendaciones basadas en señales medidas.</small></article>}
      {result.auditoria_propia?.job_refresco&&<p class="muted">Se programó una auditoría técnica actualizada de la web propia: {result.auditoria_propia.job_refresco}</p>}
    </>}
    {items.length>1&&<details><summary>Comparaciones anteriores ({items.length-1})</summary>{items.slice(1).map(item=><button type="button" class="secundario" key={item.id} onClick={()=>{setCurrent(item);onChange?.(item)}}>{new Date(item.creado).toLocaleString('es-ES')} · {item.estado} · {item.seleccion.length} competidores</button>)}</details>}
  </section>
}
