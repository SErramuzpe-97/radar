import { api, authHeaders } from '../api/client';
import { cargarDetalle, seleccionadoId, recargar, recargarMapa } from './negocios';
export async function confirmarWeb(id:number,url:string) {
 const {error}=await api.PATCH('/api/negocios/{id}/web',{params:{path:{id}},body:{url},headers:authHeaders()});
 if(error)throw new Error(error.error);if(seleccionadoId.value===id)await cargarDetalle(id);await Promise.all([recargar(),recargarMapa()]);
}
export async function descartarWeb(id:number,candidate:number) {
 const {error}=await api.DELETE('/api/negocios/{id}/web/candidatas/{candidate}',{params:{path:{id,candidate}},headers:authHeaders()});
 if(error)throw new Error(String(error));if(seleccionadoId.value===id)await cargarDetalle(id);
}
export async function descubrirWeb(id:number) {
 const {data,error}=await api.POST('/api/negocios/{id}/web/descubrir',{params:{path:{id}},headers:authHeaders()});
 if(error||!data)throw new Error('No se pudo programar la búsqueda');return data;
}
export async function consultarAuditoria(id:number) {
 const {data,error}=await api.GET('/api/negocios/{id}/auditoria-tecnica',{params:{path:{id}},headers:authHeaders()});
 if(error||!data)throw new Error('No se pudieron consultar los informes');return data;
}
export type InformeIA={id:number;negocio_id:number;url:string;creado:string;modelo:string;texto:string;evidencias:unknown};
export async function consultarInformeIA(id:number):Promise<InformeIA|null>{
 const res=await fetch(`/api/negocios/${id}/ia-informe`,{headers:authHeaders(),credentials:'same-origin'});
 if(res.status===404)return null;if(!res.ok)throw new Error('No se pudo cargar el informe IA');return res.json();
}
export async function generarInformeIA(id:number):Promise<InformeIA>{
 const res=await fetch(`/api/negocios/${id}/ia-informe`,{method:'POST',headers:authHeaders(),credentials:'same-origin'});
 if(!res.ok){const data=await res.json().catch(()=>null);throw new Error(data?.error||'No se pudo generar el informe gratuito');}return res.json();
}
export async function guardarRevisionInformeIA(id:number,texto:string):Promise<InformeIA>{
 const res=await fetch(`/api/negocios/${id}/ia-informe`,{method:'PUT',headers:{'Content-Type':'application/json',...authHeaders()},credentials:'same-origin',body:JSON.stringify({texto})});
 if(!res.ok){const data=await res.json().catch(()=>null);throw new Error(data?.error||'No se pudo guardar la revisión del informe');}return res.json();
}
export async function guardarRevisionIA(id:number,body:{decision:string;estado:string;valor?:string;motivo?:string}) {
 const {data,error}=await api.PUT('/api/negocios/{id}/ia-revisiones',{params:{path:{id}},body,headers:authHeaders()});
 if(error||!data)throw new Error('No se pudo guardar la revisión IA');return data;
}
export async function consultarRevisionHallazgo(id:number) {
 const {data,error}=await api.GET('/api/negocios/{id}/auditoria-tecnica/revisiones',{params:{path:{id}},headers:authHeaders()});
 if(error||!data)throw new Error('No se pudieron consultar las revisiones humanas');return data;
}
export async function guardarRevisionHallazgo(id:number,body:{report_id:string;finding_key:string;estado:string;motivo?:string}) {
 const {data,error}=await api.PUT('/api/negocios/{id}/auditoria-tecnica/revisiones',{params:{path:{id}},body,headers:authHeaders()});
 if(error||!data)throw new Error('No se pudo guardar la revisión humana');return data;
}
export type CaptureVisitMark={capture_id:string;estado:'pendiente'|'conservar'|'descartada';orden:number;titulo:string;que:string;impacto:string;propuesta:string;punto_x:number|null;punto_y:number|null};
export async function consultarVisitaDiagnostico(id:number):Promise<{capturas:CaptureVisitMark[];catalogo:{capture_id:string;report_id:string;key:string;width:number;page:string;fecha:string}[]}> {
 const res=await fetch(`/api/negocios/${id}/diagnostico-visita`,{headers:authHeaders(),credentials:'same-origin'});
 if(!res.ok)throw new Error('No se pudo cargar la biblioteca de capturas');return res.json();
}
export async function guardarVisitaDiagnostico(id:number,capturas:CaptureVisitMark[]) {
 const res=await fetch(`/api/negocios/${id}/diagnostico-visita`,{method:'PUT',headers:{'Content-Type':'application/json',...authHeaders()},credentials:'same-origin',body:JSON.stringify({capturas})});
 if(!res.ok)throw new Error('No se pudo guardar la selección de capturas');return res.json();
}
export async function lanzarAuditoria(id:number) {
 const {error}=await api.POST('/api/negocios/{id}/auditoria-tecnica',{params:{path:{id}},headers:authHeaders()});
 if(error)throw new Error(error.error);return consultarAuditoria(id);
}

export type SeoCompetidor={negocio_id:number|null;nombre:string;municipio:string|null;url:string;puntuacion_ia:number|null;fuentes:unknown[]};
export type SeoComparacion={id:string;negocio_id:number;url_propia:string;estado:string;sugerencias:SeoCompetidor[];seleccion:SeoCompetidor[];resultado:any;creado:string;actualizado:string;job_id:string|null;error:string|null};
async function seoFetch<T>(url:string,init?:RequestInit):Promise<T>{const response=await fetch(url,{...init,headers:{...(init?.body?{'Content-Type':'application/json'}:{}),...authHeaders(),...init?.headers},credentials:'same-origin'});if(!response.ok){const body=await response.json().catch(()=>null);throw new Error(body?.error||'No se pudo completar la comparación SEO');}return response.json();}
export const consultarComparacionesSeo=(id:number)=>seoFetch<SeoComparacion[]>(`/api/negocios/${id}/seo-competidores`);
export const consultarComparacionSeo=(id:number,analysisId:string)=>seoFetch<SeoComparacion>(`/api/negocios/${id}/seo-competidores/${encodeURIComponent(analysisId)}`);
export const sugerirCompetidoresSeo=(id:number)=>seoFetch<SeoComparacion>(`/api/negocios/${id}/seo-competidores/sugerencias`,{method:'POST'});
export const iniciarComparacionSeo=(id:number,analysisId:string,competidores:SeoCompetidor[])=>seoFetch<{id:string}>(`/api/negocios/${id}/seo-competidores/${encodeURIComponent(analysisId)}/analizar`,{method:'POST',body:JSON.stringify({competidores})});

export async function consultarJob(id:string) {
 const {data,error}=await api.GET('/api/jobs/{id}',{params:{path:{id}},headers:authHeaders()});
 if(error||!data)throw new Error('No se pudo consultar el trabajo de búsqueda');return data;
}
