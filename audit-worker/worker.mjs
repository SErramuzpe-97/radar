import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {gzip} from 'node:zlib';
import lighthouse from 'lighthouse';
import desktopConfig from 'lighthouse/core/config/desktop-config.js';
import * as chromeLauncher from 'chrome-launcher';
import puppeteer from 'puppeteer-core';
import {createProxy,destination,publicFetch} from './public-proxy.mjs';
import {lighthouseResult,siteoneResult,supplementary,mergeRendered} from './normalize.mjs';

const exec=promisify(execFile), zip=promisify(gzip);
const require=createRequire(import.meta.url);
const axePath=require.resolve('axe-core/axe.min.js');
const base=process.env.RADAR_AUDIT_DIR||'/var/lib/radar-audits';
const siteone=process.env.RADAR_SITEONE_BIN||'/opt/radar-audit/siteone-crawler';
const chromePath=process.env.RADAR_CHROME_WRAPPER||'/opt/radar-audit/chrome-wrapper';
const now=()=>Math.floor(Date.now()/1000);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
for(const folder of ['requests','results','artifacts','tmp','cache'])await fs.mkdir(path.join(base,folder),{recursive:true,mode:0o770});
let current=null,ready=false,versions={},lastError=null;
async function atomic(file,value){const tmp=file+'.tmp';await fs.writeFile(tmp,JSON.stringify(value),{mode:0o660});await fs.rename(tmp,file);}
async function heartbeat(){await atomic(path.join(base,'heartbeat.json'),{timestamp:now(),ready,current,versions,error:lastError});}
async function tools(){
 const pkg=JSON.parse(await fs.readFile(new URL('./node_modules/lighthouse/package.json',import.meta.url),'utf8'));
 const s=await exec(siteone,['--version'],{timeout:10000,maxBuffer:100000}).catch(e=>{if(e.stdout?.includes('2.5.1'))return {stdout:e.stdout};throw e;});
 const c=await exec(chromePath,['--version'],{timeout:10000,maxBuffer:100000});
 if(pkg.version!=='13.6.0'||!s.stdout.includes('2.5.1'))throw new Error('Las versiones instaladas no coinciden con Lighthouse 13.6.0 y SiteOne 2.5.1');
 versions={lighthouse:pkg.version,siteone:s.stdout.replace(/\x1b\[[0-9;]*m/g,'').trim(),chrome:c.stdout.trim(),node:process.version};ready=true;lastError=null;
}
try{await tools();}catch(e){lastError=e.message;console.error('Worker no disponible:',lastError);}
await heartbeat();const beat=setInterval(()=>heartbeat().catch(e=>console.error('heartbeat',e.message)),5000);

async function run(command,args,timeout,env){
 return new Promise((resolve,reject)=>{
  const p=spawn(command,args,{cwd:base,env,detached:true,stdio:['ignore','pipe','pipe']});let log='';let timedOut=false;
  const collect=c=>{if(log.length<150000)log+=c.toString();};p.stdout.on('data',collect);p.stderr.on('data',collect);
  const timer=setTimeout(()=>{timedOut=true;try{process.kill(-p.pid,'SIGTERM');}catch{}setTimeout(()=>{try{process.kill(-p.pid,'SIGKILL');}catch{}},3000).unref();},timeout);
  p.on('error',e=>{clearTimeout(timer);reject(e);});p.on('close',code=>{clearTimeout(timer);resolve({code,timedOut,log});});
 });
}
async function original(id,kind,content){
 const filename=`${id}.${kind}.gz`;await fs.writeFile(path.join(base,'artifacts',filename),await zip(content),{mode:0o640});return filename;
}
async function responsiveAudit(req,env,pages=[]){
 const files={},issues=[],screenshots=[],origin=new URL(req.url).origin;
 const rest=pages.map(p=>p.url||p).filter((u,i,a)=>{try{return a.indexOf(u)===i&&new URL(u).origin===origin}catch{return false}})
  .filter(u=>u!==req.url).sort((a,b)=>Number(/contact|contacto|about|nosotr|servici|menu|carta|reserv|donac|proyecto|socios/i.test(b))-Number(/contact|contacto|about|nosotr|servici|menu|carta|reserv|donac|proyecto|socios/i.test(a)));
 const targets=[req.url,...rest].slice(0,3);
 let browser;
 try{
  browser=await puppeteer.launch({executablePath:chromePath,headless:true,env,args:['--disable-dev-shm-usage','--renderer-process-limit=2','--no-first-run','--no-default-browser-check']});
  const page=await browser.newPage();page.setDefaultNavigationTimeout(10000);
  let expected=targets.length*3;
  for(const [pageIndex,url] of targets.entries()){
   const widths=[390,768,1366];
   for(let wi=0;wi<widths.length;wi++){
    const width=widths[wi];
    if(timeLeft(req,60000)<5000)break;
    await page.setViewport({width,height:width===1366?768:844,deviceScaleFactor:1,isMobile:width<768,hasTouch:width<768});
    let response=null;
    try{response=await page.goto(url,{waitUntil:'domcontentloaded',timeout:Math.min(10000,timeLeft(req,10000))});await new Promise(r=>setTimeout(r,250));}
    catch(e){issues.push({code:'responsive-navigation',title:'No se pudo cargar la página a '+width+' px',category:'responsive',severity:'info',confidence:'low',url,details:e.message});continue;}
    const geometry=await page.evaluate(()=>{const root=document.documentElement,client=root.clientWidth,scroll=root.scrollWidth;const nodes=Array.from(document.body?.querySelectorAll('*')||[]).map(el=>{const r=el.getBoundingClientRect(),style=getComputedStyle(el);return {tag:el.tagName.toLowerCase(),id:el.id||null,classes:typeof el.className==='string'?el.className.slice(0,100):'',left:Math.round(r.left),right:Math.round(r.right),width:Math.round(r.width),position:style.position}}).filter(x=>x.width>0&&x.right>client+2&&x.left>=-2&&x.position!=='fixed'&&x.position!=='absolute').sort((a,b)=>b.right-a.right).slice(0,5);return {client,scroll,overflow:scroll>client+1,nodes}});
    const key='responsive-'+(pageIndex+1)+'-'+width;
    files[key]=await original(req.id,key,await page.screenshot({type:'png'}));screenshots.push({page:url,width,key});
    if(geometry.overflow)issues.push({code:'horizontal-overflow',title:'Desbordamiento horizontal observado a '+width+' px',category:'responsive',severity:width<=390?'high':'medium',confidence:'high',url,viewport:{width,height:width===1366?768:844},selector:geometry.nodes[0]?geometry.nodes[0].tag+(geometry.nodes[0].id?'#'+geometry.nodes[0].id:'')+(geometry.nodes[0].classes?'.'+geometry.nodes[0].classes.trim().split(/\s+/).join('.'): ''):null,details:'documentElement.scrollWidth='+geometry.scroll+' px; clientWidth='+geometry.client+' px; elementos fuera del ancho: '+JSON.stringify(geometry.nodes)});
    if(width===390&&geometry.overflow){widths.splice(wi+1,0,320);expected++;}
    if(response&&response.status()>=400)issues.push({code:'responsive-http',title:'La página devolvió HTTP '+response.status()+' a '+width+' px',category:'availability',severity:'medium',confidence:'high',url,viewport:{width},details:'Estado HTTP de la navegación.'});
    if(width===390){
     await page.addScriptTag({path:axePath});
     const axe=await page.evaluate(async()=>await window.axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21a','wcag21aa','wcag22aa']}}));
     for(const v of axe.violations.slice(0,80))for(const node of v.nodes.slice(0,20))issues.push({code:v.id,title:v.help,category:'accessibility',severity:v.impact==='critical'?'critical':v.impact==='serious'?'high':v.impact==='moderate'?'medium':'low',confidence:'high',url,selector:(node.target||[]).slice(0,3).join(', '),details:node.failureSummary||node.summary||'',recommendation:v.helpUrl,tags:v.tags,impact:v.impact,engine:'axe-core '+axe.version});
     for(const v of axe.incomplete.slice(0,80))issues.push({code:'axe-manual-'+v.id,title:'Revisión manual necesaria: '+v.help,category:'accessibility',severity:'info',confidence:'low',review:'pending',url,selector:v.nodes?.[0]?.target?.join(', ')||null,details:'axe-core marcó este caso como incompleto; no es una infracción confirmada.',engine:'axe-core '+axe.version});
    }
   }
   if(timeLeft(req,60000)<5000)break;
  }
  return {files,issues,screenshots,expected,targets,version:'Puppeteer-core 25.16.0 · axe-core 4.14.0'};
 }finally{if(browser)await browser.close().catch(()=>{});}
}
function timeLeft(req,cap){return Math.max(500,Math.min(cap,(req.deadline-now())*1000));}
async function lighthousePhase(req,env){
 let chrome;let timer;
 try {
  chrome=await chromeLauncher.launch({chromePath,envVars:env,chromeFlags:['--headless=new','--disable-dev-shm-usage','--renderer-process-limit=2','--no-first-run','--no-default-browser-check'],logLevel:'silent'});
  const config={extends:'lighthouse:default',settings:{...(req.device==='desktop'?desktopConfig.settings:{}),formFactor:req.device,onlyCategories:['performance','accessibility','best-practices','seo'],screenEmulation:{mobile:req.device==='mobile',width:req.device==='mobile'?390:1366,height:req.device==='mobile'?844:768,deviceScaleFactor:req.device==='mobile'?1.75:1,disabled:false},maxWaitForLoad:30000,maxWaitForFcp:20000}};
  const timeout=new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Tiempo de medición Lighthouse agotado (110 s)')),timeLeft(req,110000));});
  const result=await Promise.race([lighthouse(req.url,{port:chrome.port,output:['json','html'],locale:'es',logLevel:'error'},config),timeout]);
  if(!result?.lhr)throw new Error('Lighthouse no devolvió resultados');
  const normalized=lighthouseResult(result.lhr);const reports=Array.isArray(result.report)?result.report:[JSON.stringify(result.lhr),result.report];
  const files={html:await original(req.id,'html',reports[1])};
  return {status:result.lhr.runtimeError?'no_disponible':'completo',version:result.lhr.lighthouseVersion,normalized,files,error:result.lhr.runtimeError?.message||null};
 }finally{clearTimeout(timer);if(chrome)await Promise.resolve(chrome.kill()).catch(()=>{});}
}
async function siteonePhase(req,env){
 const dir=path.join(base,'tmp',req.id);await fs.mkdir(dir,{recursive:true,mode:0o770});
 const args=(json,html,max,rendered)=>[
  `--url=${req.url}`,`--device=${req.device}`,'--workers=1','--max-reqs-per-sec=1','--memory-limit=256M',`--max-visited-urls=${max}`,'--max-depth=2','--timeout=8',
  `--proxy=${env.RADAR_BROWSER_PROXY.replace('http://','')}`,
  `--output-json-file=${json}`,`--output-html-report=${html}`,'--output-text-file=',
  ...(rendered?['--browser',`--browser-path=${chromePath}`,'--browser-workers=1','--browser-wait=domcontentloaded','--browser-wait-extra=500','--browser-timeout=8',`--screenshot-viewport=${req.device==='mobile'?'390x844':'1366x768'}`]:[])
 ];
 const jsonPath=path.join(dir,'report.json'),htmlPath=path.join(dir,'report.html');
 const runInfo=await run(siteone,args(jsonPath,htmlPath,40,false),timeLeft(req,180000),env);
 let d;try{d=JSON.parse(await fs.readFile(jsonPath,'utf8'));}catch{throw new Error(runInfo.timedOut?'SiteOne alcanzó el límite de tiempo sin poder generar informe':`SiteOne no generó informe (código ${runInfo.code}): ${runInfo.log.slice(-1800)}`);}
 let normalized=siteoneResult(d,req.url,40,false);
 const files={html:await original(req.id,'html',await fs.readFile(htmlPath))};
 try{const page=await publicFetch(req.url);if(page.status>=200&&page.status<300)normalized.issues.push(...supplementary(page.body,page.url));}catch(e){normalized.warnings.push('Comprobaciones complementarias no disponibles: '+e.message);}
 const needRender=normalized.coverage.html_pages<=1 || normalized.issues.some(i=>['h1-missing','title-missing','pages-without-h1'].includes(i.code));
 if(needRender && req.deadline-now()>15){
  // Una página por invocación permite conservar cada comprobación terminada
  // incluso cuando otra URL JS consume el presupuesto restante.
  const priority=url=>/contact|encuentra|about|nosotr|servici|menu/i.test(url)?0:1;
  const targets=[req.url,...normalized.pages.map(p=>p.url).sort((a,b)=>priority(a)-priority(b))].filter((u,i,a)=>a.indexOf(u)===i&&new URL(u).origin===new URL(req.url).origin).slice(0,3);
  const renderEnd=Math.min(req.deadline,now()+30);let renderedCount=0;
  for(const [index,target] of targets.entries()){
   if(renderEnd-now()<5){normalized.coverage.rendered_incomplete=true;break;}
   const renderedJson=path.join(dir,`rendered-${index}.json`),renderedHtml=path.join(dir,`rendered-${index}.html`);
   const renderArgs=args(renderedJson,renderedHtml,1,true).map(a=>a.startsWith('--url=')?`--url=${target}`:a);
   const render=await run(siteone,renderArgs,Math.max(1000,Math.min(10000,(renderEnd-now())*1000)),env);
   try{
    const rd=JSON.parse(await fs.readFile(renderedJson,'utf8'));const rendered=siteoneResult(rd,target,1,true);
    if(!rendered.coverage.html_pages)throw new Error('Sin HTML renderizado');
    normalized=mergeRendered(normalized,rendered);renderedCount+=rendered.coverage.html_pages;normalized.coverage.rendered_pages=renderedCount;
    const key=index===0?'rendered':`rendered_${index+1}`;
    // El rastreo renderizado alimenta los hallazgos normalizados; solo se archiva
    // el informe HTML original del motor para descarga y vista previa.
   }catch{normalized.coverage.rendered_incomplete=true;normalized.warnings.push(`Comprobación JavaScript no disponible para ${target}${render.timedOut?' (límite de tiempo)':''}.`);}
  }
  if(normalized.coverage.rendered_incomplete)normalized.warnings.push('Cobertura JavaScript parcial; los hallazgos HTML no contrastados requieren revisión.');
 }

 if(req.device==='mobile'){
  try{
   const diagnostic=await responsiveAudit(req,env,normalized.pages||[]);
   normalized.issues.push(...diagnostic.issues);
   normalized.responsive={status:diagnostic.screenshots.length===diagnostic.expected?'complete':'partial',profile:'Chromium emulado; hasta cinco páginas; viewport CSS y táctil en anchos menores de 768 px',pages:diagnostic.targets,screenshots:diagnostic.screenshots,measured_at:new Date().toISOString(),engine:diagnostic.version};
   Object.assign(files,diagnostic.files);
   if(diagnostic.screenshots.length<diagnostic.expected)normalized.warnings.push('Matriz responsive incompleta por el límite de ejecución.');
  }catch(e){normalized.responsive={status:'unavailable',screenshots:[]};normalized.warnings.push('Comprobación responsive/axe no disponible: '+e.message);}
 }
 normalized.environment={...normalized.environment,device:req.device,viewport:req.device==='mobile'?{width:390,height:844}:{width:1366,height:768}};
 const issueRank={critical:0,high:1,medium:2,low:3,info:4};
 normalized.issues.sort((a,b)=>(issueRank[a.severity]??5)-(issueRank[b.severity]??5)||(a.category==='responsive'?0:1)-(b.category==='responsive'?0:1));
 const omitted= Math.max(0,normalized.issues.length-200);
 normalized.issues=normalized.issues.slice(0,200);
 if(omitted){normalized.coverage.issues_omitted=omitted;normalized.warnings.push('Hallazgos truncados por el límite de 200; '+omitted+' no se muestran en este informe.');}
 const status=!normalized.coverage.html_pages?'no_disponible':normalized.coverage.limit_reached||runInfo.timedOut||normalized.coverage.rendered_incomplete||omitted>0||['partial','unavailable'].includes(normalized.responsive?.status)?'parcial':'completo';
 await fs.rm(dir,{recursive:true,force:true});return {status,version:d.crawler?.version||'2.5.1',normalized,files,error:null};
}
async function cleanup(){
 const folder=path.join(base,'artifacts');let files=[];
 const pinnedFolder=path.join(folder,'pinned');await fs.mkdir(pinnedFolder,{recursive:true,mode:0o770});
 for(const name of await fs.readdir(pinnedFolder)){const marker=path.join(pinnedFolder,name);const st=await fs.lstat(marker);if(st.isFile()&&name.endsWith('.keep')&&Date.now()-st.mtimeMs>90*86400000)await fs.unlink(marker).catch(()=>{});}
 for(const name of await fs.readdir(folder)){const full=path.join(folder,name);const st=await fs.lstat(full);if(!st.isFile()||!name.endsWith('.gz'))continue;if(!/^[a-zA-Z0-9-]+\.(html|responsive-\d+-\d+)\.gz$/.test(name)){await fs.unlink(full).catch(()=>{});continue;}let pinned=false;try{const keep=await fs.stat(path.join(pinnedFolder,name+'.keep'));pinned=Date.now()-keep.mtimeMs<=90*86400000;}catch{}files.push({full,size:st.size,mtime:st.mtimeMs,pinned});}
 files.sort((a,b)=>a.mtime-b.mtime);let total=files.reduce((s,f)=>s+f.size,0);
 for(const file of files){if(!file.pinned&&(Date.now()-file.mtime>7*86400000||total>2*1024**3)){await fs.unlink(file.full).catch(()=>{});total-=file.size;}}
}
await cleanup();
async function handle(req){
 if(!/^[a-zA-Z0-9-]{1,150}$/.test(req.id)||!['lighthouse','siteone'].includes(req.engine)||!['mobile','desktop'].includes(req.device))throw new Error('Solicitud inválida');
 if(req.deadline<=now())throw new Error('El presupuesto de ejecución ha caducado');
 if((req.attempts||0)>2)throw new Error('El proceso se interrumpió repetidamente; requiere reintento del trabajo');
 global.gc?.();await destination(req.url);const proxy=await createProxy();
 const env={PATH:process.env.PATH,LANG:'C.UTF-8',HOME:base,TZ:'Europe/Madrid',TMPDIR:path.join(base,'tmp'),XDG_CACHE_HOME:path.join(base,'cache'),RADAR_BROWSER_PROXY:proxy.url,RADAR_CHROME_REAL:process.env.RADAR_CHROME_REAL||'/opt/radar-audit/chrome/chrome-linux64/chrome'};
 try {
  const r=req.engine==='lighthouse'?await lighthousePhase(req,env):await siteonePhase(req,env);
  r.normalized.environment={...r.normalized.environment,worker:{node:process.version,platform:os.platform(),arch:os.arch(),chrome:versions.chrome},measured_at:new Date().toISOString()};return r;
 }finally{await proxy.close();global.gc?.();}
}
// Recuperar solicitudes interrumpidas. Las fases terminadas están registradas por Radar.
for(const name of await fs.readdir(path.join(base,'requests'))){if(name.endsWith('.working')){const file=path.join(base,'requests',name);try{const req=JSON.parse(await fs.readFile(file,'utf8'));req.attempts=(req.attempts||0)+1;await atomic(file.replace(/\.working$/,'.json'),req);await fs.unlink(file);}catch{await fs.unlink(file).catch(()=>{});}}}
console.log('Radar audit worker listo:',JSON.stringify(versions));
while(true){
 if(!ready){await sleep(30000);try{await tools();}catch(e){lastError=e.message;}await heartbeat();continue;}
 const names=(await fs.readdir(path.join(base,'requests'))).filter(n=>n.endsWith('.json')).sort();
 if(!names.length){await sleep(1000);continue;}
 const name=names[0],file=path.join(base,'requests',name),working=file.replace(/\.json$/,'.working');
 let req;try{await fs.rename(file,working);req=JSON.parse(await fs.readFile(working,'utf8'));}catch(e){lastError='Solicitud no legible: '+e.message;await fs.unlink(working).catch(()=>{});continue;}
 current=req.id;await heartbeat();const started=Date.now();let result;
 try{result=await handle(req);}catch(e){result={status:'error',version:req.engine==='lighthouse'?'13.6.0':'2.5.1',normalized:{scores:{},metrics:[],issues:[],coverage:{html_pages:0,rendered_pages:0},warnings:[]},files:{},error:e.message};}
 result.id=req.id;result.elapsed_ms=Date.now()-started;
 await atomic(path.join(base,'results',`${req.id}.json`),result);await fs.unlink(working).catch(()=>{});await cleanup();
 console.log(JSON.stringify({id:req.id,engine:req.engine,device:req.device,status:result.status,ms:result.elapsed_ms,error:result.error}));current=null;await heartbeat();
}
