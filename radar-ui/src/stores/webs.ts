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

export async function consultarJob(id:string) {
 const {data,error}=await api.GET('/api/jobs/{id}',{params:{path:{id}},headers:authHeaders()});
 if(error||!data)throw new Error('No se pudo consultar el trabajo de búsqueda');return data;
}
