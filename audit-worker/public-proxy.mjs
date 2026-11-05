import http from 'node:http';
import https from 'node:https';
import net from 'node:net';
import dns from 'node:dns/promises';
import ipaddr from 'ipaddr.js';

export function publicAddress(raw) {
  try { let a=ipaddr.parse(raw); if(a.kind()==='ipv6' && a.isIPv4MappedAddress()) a=a.toIPv4Address(); return a.range()==='unicast'; } catch {return false;}
}
export async function destination(raw) {
 const u=new URL(raw);const host=u.hostname.replace(/^\[|\]$/g,'');const port=Number(u.port||(u.protocol==='https:'?443:80));
 if(!['http:','https:'].includes(u.protocol)||u.username||u.password||![80,443].includes(port)||/^(localhost)$|\.(localhost|local)$/i.test(host))throw new Error('URL privada, no HTTP(S), con credenciales o puerto no permitido');
 const ips=net.isIP(host)?[{address:host,family:net.isIP(host)}]:await dns.lookup(host,{all:true,verbatim:true});
 if(!ips.length||ips.some(a=>!publicAddress(a.address)))throw new Error('Destino privado o reservado');
 return {u,host,port,ip:ips.find(a=>a.family===4)||ips[0]};
}
export async function publicFetch(raw,maxBytes=2*1024*1024) {
 let current=raw;
 for(let i=0;i<6;i++) {
  const {u,host,port,ip}=await destination(current);
  const data=await new Promise((resolve,reject)=>{
   const mod=u.protocol==='https:'?https:http;
   const req=mod.request({protocol:u.protocol,hostname:host,port,path:u.pathname+u.search,servername:host,lookup:(_h,_o,cb)=>_o?.all?cb(null,[ip]):cb(null,ip.address,ip.family),headers:{'User-Agent':'RadarNavarra/0.2 (public website audit)'}},r=>{
    const chunks=[];let size=0;r.on('data',c=>{size+=c.length;if(size>maxBytes){req.destroy(new Error('Respuesta demasiado grande'));}else chunks.push(c);});r.on('end',()=>resolve({status:r.statusCode,headers:r.headers,body:Buffer.concat(chunks).toString(),url:current}));r.on('error',reject);
   });req.setTimeout(15000,()=>req.destroy(new Error('Tiempo de respuesta agotado')));req.on('error',reject);req.end();
  });
  if(data.status>=300&&data.status<400&&data.headers.location){current=new URL(data.headers.location,current).href;continue;}return data;
 }throw new Error('Demasiadas redirecciones');
}
export async function createProxy() {
 const sockets=new Set();
 const server=http.createServer(async(req,res)=>{
  try {
   if(!['GET','HEAD','OPTIONS'].includes(req.method))throw new Error('Solo peticiones de lectura');
   const {u,host,port,ip}=await destination(req.url);
   if(u.protocol!=='http:')throw new Error('HTTPS requiere CONNECT');
   const headers={...req.headers,host:u.host};delete headers['proxy-connection'];delete headers['proxy-authorization'];
   const upstream=http.request({hostname:ip.address,family:ip.family,port,method:req.method,path:u.pathname+u.search,headers},r=>{res.writeHead(r.statusCode,r.headers);r.pipe(res);});
   upstream.setTimeout(30000,()=>upstream.destroy(new Error('timeout')));upstream.on('error',()=>{if(!res.headersSent)res.writeHead(502);res.end();});req.pipe(upstream);
  }catch{res.writeHead(403);res.end('Destino no permitido');}
 });
 server.on('connect',async(req,client,head)=>{
  try {
   const {port,ip}=await destination('https://'+req.url);
   const remote=net.connect({host:ip.address,port,family:ip.family});sockets.add(remote);remote.on('close',()=>sockets.delete(remote));
   remote.on('connect',()=>{client.write('HTTP/1.1 200 Connection Established\r\n\r\n');if(head.length)remote.write(head);client.pipe(remote);remote.pipe(client);});
   remote.setTimeout(45000,()=>remote.destroy());remote.on('error',()=>client.destroy());client.on('error',()=>remote.destroy());client.on('close',()=>remote.destroy());
  }catch{client.end('HTTP/1.1 403 Forbidden\r\n\r\n');}
 });
 server.on('connection',s=>{sockets.add(s);s.on('close',()=>sockets.delete(s));});
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
 return {url:`http://127.0.0.1:${server.address().port}`,close:async()=>{for(const s of sockets)s.destroy();await new Promise(r=>server.close(r));}};
}
