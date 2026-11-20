import {useEffect,useState} from 'preact/hooks';
import {detalle,cargarDetalle} from '../stores/negocios';
import {confirmarWeb,descartarWeb,descubrirWeb,consultarJob,guardarRevisionIA} from '../stores/webs';
import {Desplegable} from './Desplegable';
export function WebNegocio(){
 const d=detalle.value!;const w=d.web_info,n=d.negocio;const confirmed=w.url_confirmada;const automatic=w.origen==='verificacion_automatica';const ai:any=w.evaluacion_ia;
 const [url,setUrl]=useState(''),[ocupado,setOcupado]=useState(false),[mensaje,setMensaje]=useState('');
 const [decisionIA,setDecisionIA]=useState('categoria'),[estadoIA,setEstadoIA]=useState('revisada'),[valorIA,setValorIA]=useState(''),[motivoIA,setMotivoIA]=useState('');
 useEffect(()=>{setUrl('');setMensaje('');},[n.id,confirmed]);
 const actuar=async(fn:()=>Promise<unknown>)=>{setOcupado(true);setMensaje('');try{await fn();await cargarDetalle(n.id);}catch(e){setMensaje(String(e));}finally{setOcupado(false);}};
 const buscar=async()=>{const job=await descubrirWeb(n.id);setMensaje('Búsqueda en cola: 0/'+job.total+' negocios.');
  for(let i=0;i<120;i++){await new Promise(r=>setTimeout(r,5000));if(detalle.value?.negocio.id!==n.id)return;
   const current=await consultarJob(job.id);setMensaje(current.estado==='en_curso'?'Buscando web ('+current.hechos+'/'+current.total+').':'Búsqueda en cola ('+current.hechos+'/'+current.total+').');
   if(['terminado','error'].includes(current.estado)){await cargarDetalle(n.id);setMensaje(current.estado==='error'?'La búsqueda terminó con incidencias: '+(current.error??'revisa el job'):'Búsqueda terminada; fichas y sugerencias actualizadas.');return;}
  }setMensaje('La búsqueda sigue en cola. Puedes continuar usando Radar; el job se puede consultar en Buscar empresas.');return job;};
 return <section class="web-negocio"><h3>Presencia web</h3>
  {confirmed?<p class="web-confirmada"><span class="badge potencial-bajo">{automatic?'Web verificada automáticamente':'Web confirmada por ti'}</span> <a href={confirmed} target="_blank" rel="noreferrer">{confirmed}</a></p>:<p><span class="badge potencial-medio">{w.candidatas.length?'Web encontrada · verificándose':'Web no localizada'}</span> <small>{w.candidatas.length?'La comprobación continúa automáticamente.':'Requiere una búsqueda; no significa que no exista.'}</small></p>}
  {w.candidatas.map(c=><article class="web-candidata" key={c.id}>
   <span class="badge potencial-medio">Web sugerida · Sin confirmar</span>
   <p><a href={c.url} target="_blank" rel="noreferrer">{c.titulo||c.url}</a></p>
   <small class="muted">Fuente: {c.proveedor}</small>
   <div class="acciones">{!['red_social','directorio'].includes(c.tipo)&&<button disabled={ocupado} onClick={()=>void actuar(()=>confirmarWeb(n.id,c.url))}>Confirmar web propia</button>}<button class="secundario" disabled={ocupado} onClick={()=>void actuar(()=>descartarWeb(n.id,c.id))}>Descartar sugerencia</button></div>
  </article>)}
  {ai&&<details class="web-ia-evaluacion"><summary>Evaluación OpenRouter · {ai.modelo||'modelo gratuito'}</summary>{ai.ejecucion?.estado==='obsoleta'&&<p class="muted">Esta evaluación quedó obsoleta porque la ficha cambió; no se aplicó.</p>}<p>Categoría: {ai.respuesta?.answers?.category?.choice||'sin determinar'}</p><p>Actividad: {ai.respuesta?.answers?.activity?.choice||'sin determinar'} · Organización: {ai.respuesta?.answers?.organization?.choice||'sin determinar'}</p>{(ai.respuesta?.annotations||[]).map((source:any,index:number)=><p key={source.url||index}><a href={source.url} target="_blank" rel="noreferrer">{source.title||source.url}</a></p>)}</details>}
  {ai&&<section class="web-ia-evaluacion"><h4>Revisión humana</h4>{(w.revisiones_ia||[]).map((review:any)=><p key={review.decision}><b>{review.decision}</b>: {review.estado}{review.valor?` · ${review.valor}`:''}{review.motivo?` · ${review.motivo}`:''}</p>)}<form class="web-correccion" onSubmit={e=>{e.preventDefault();void actuar(()=>guardarRevisionIA(n.id,{decision:decisionIA,estado:estadoIA,valor:valorIA||undefined,motivo:motivoIA||undefined}));}}><label>Dato revisado<select value={decisionIA} onChange={e=>setDecisionIA(e.currentTarget.value)}><option value="categoria">Categoría</option><option value="actividad">Actividad</option><option value="organizacion">Cadena u organización</option><option value="web">Web</option></select></label><label>Revisión<select value={estadoIA} onChange={e=>setEstadoIA(e.currentTarget.value)}><option value="revisada">Confirmar evaluación</option><option value="corregida">Corregir</option><option value="rechazada">Rechazar evaluación</option></select></label><label>Valor corregido<input value={valorIA} onInput={e=>setValorIA(e.currentTarget.value)} placeholder={decisionIA==='categoria'?'Clave de categoría, por ejemplo comercio':'Valor revisado'} /></label><label>Motivo<textarea value={motivoIA} onInput={e=>setMotivoIA(e.currentTarget.value)} maxLength={1000} /></label><button disabled={ocupado}>{ocupado?'Guardando…':'Guardar revisión'}</button></form></section>}
  {!confirmed&&<><p class="muted"><small>{w.aviso||(!w.buscador_configurado?'Búsqueda gratuita de OpenRouter sin configurar. Añade fuentes públicas o introduce la URL.':'OpenRouter gratuito · '+w.creditos_restantes+' solicitudes IA restantes hoy.')}</small></p><div class="acciones"><button disabled={ocupado} onClick={()=>void actuar(buscar)}>Buscar web</button></div></>}
  <Desplegable className="web-add" titulo={confirmed?'Cambiar web confirmada':'Añadir web'}>
   <form class="web-correccion" onSubmit={e=>{e.preventDefault();void actuar(async()=>{await confirmarWeb(n.id,url);setUrl('');setMensaje('Web confirmada y auditoría programada.');});}}>
    <label htmlFor={'web-correcta-'+n.id}>{confirmed?'Corregir URL confirmada':'Introducir web propia'}</label><input id={'web-correcta-'+n.id} type="url" placeholder="https://…" value={url} required onInput={e=>setUrl(e.currentTarget.value)}/><button disabled={ocupado||!url}>Confirmar URL</button>
   </form>
  </Desplegable>{mensaje&&<p role="status">{mensaje}</p>}
 </section>;
}
