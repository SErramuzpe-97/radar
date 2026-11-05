import * as cheerio from 'cheerio';
const clean=s=>String(s??'').replace(/<[^>]*>/g,' ').replace(/\s+/g,' ').trim().slice(0,2000);
const urlFor=(path,base)=>{try{return new URL(path||'/',base).href;}catch{return base;}};
export function lighthouseResult(lhr) {
 const scores=Object.fromEntries(Object.entries(lhr.categories||{}).map(([key,v])=>[key,v.score==null?null:Math.round(v.score*100)]));
 const metricKeys={'largest-contentful-paint':'LCP','cumulative-layout-shift':'CLS','first-contentful-paint':'FCP','speed-index':'Speed Index','total-blocking-time':'TBT'};
 const metrics=Object.entries(metricKeys).map(([id,label])=>({code:id,label,value:lhr.audits?.[id]?.numericValue??null,unit:id==='cumulative-layout-shift'?'ratio':'ms',display:lhr.audits?.[id]?.displayValue??null}));
 const categories=new Map();for(const [key,c] of Object.entries(lhr.categories||{}))for(const ref of c.auditRefs||[])if(!categories.has(ref.id))categories.set(ref.id,key);
 const issues=Object.values(lhr.audits||{}).filter(a=>a.score!=null&&a.score<1&&!['informative','notApplicable','manual'].includes(a.scoreDisplayMode)).slice(0,150).map(a=>{const items=a.details?.items||[];const nodes=items.slice(0,20).map(n=>({selector:n.node?.selector||n.selector||null,html:clean(n.node?.snippet||n.node?.nodeLabel||''),summary:clean(n.failureSummary||n.explanation||n.label||'')}));return {code:a.id,title:clean(a.title),details:clean(a.displayValue||a.description),category:categories.get(a.id)||'performance',severity:a.score===0?'medium':'low',confidence:nodes.length?'high':'medium',url:lhr.finalDisplayedUrl||lhr.finalUrl||lhr.requestedUrl,selector:nodes[0]?.selector||null,nodes,recommendation:clean(a.description),engine:'Lighthouse '+lhr.lighthouseVersion}});
 return {scores,metrics,issues,coverage:{html_pages:lhr.runtimeError?0:1,rendered_pages:lhr.runtimeError?0:1,discovered:1,blocked:0,omitted:0},environment:{user_agent:lhr.userAgent,chrome:lhr.environment?.hostUserAgent,benchmark_index:lhr.environment?.benchmarkIndex,lighthouse:lhr.lighthouseVersion,profile:lhr.configSettings?.formFactor,throttling:lhr.configSettings?.throttling,screen:lhr.configSettings?.screenEmulation},warnings:lhr.runWarnings||[],runtime_error:lhr.runtimeError||null};
}
export function siteoneResult(d,base,limit=100,rendered=false) {
 const results=d.results||[];const rows=d.tables?.seo?.rows||[];const issues=[];
 const push=(code,title,url,severity='medium',details='',category='seo',hosting=false)=>issues.push({code,title,url:urlFor(url,base),severity,details:clean(details),category,hosting_evidence:hosting});
 for(const row of rows) {
  const url=row.url||urlFor(row.urlPathAndQuery,base);
  if(!clean(row.title))push('title-missing','Falta el título de la página',url,'high');
  if(!clean(row.description))push('description-missing','Falta la metadescripción',url);
  const h1=clean(row.h1);if(!h1||h1==='0')push('h1-missing','No se detectó un H1',url);
  const indexing=clean(row.indexing);if(/noindex|blocked|denied|disallow/i.test(indexing)||String(row.robotsIndex)==='0'||row.deniedByRobotsTxt===true||row.deniedByRobotsTxt==='true')push('indexing-review','Revisar las directivas de indexabilidad',url,'low',indexing);
  const canonical=row.canonical||row.canonicalUrl;
  if(canonical){try{if(new URL(canonical,url).hostname!==new URL(base).hostname)push('canonical-external','Canonical hacia otro dominio: revisar intención',url,'low',canonical);}catch{push('canonical-invalid','Canonical no válido',url);}}
 }
 for(const row of d.tables?.['seo-headings']?.rows||[])if(Number(row.headingsErrorsCount)>0)push('heading-structure','Revisar jerarquía de encabezados',row.urlPathAndQuery,'medium',row.headings);
 for(const kind of ['non-unique-titles','non-unique-descriptions'])for(const row of d.tables?.[kind]?.rows||[])if(Number(row.count)>1)push(kind,kind.includes('titles')?'Títulos repetidos entre páginas':'Metadescripciones repetidas entre páginas',base,'medium',`${row.count} páginas: ${row.title||row.description||''}`);
 for(const row of d.tables?.redirects?.rows||[])push('redirect-review','Redirección observada: revisar destino',row.url||row.urlPathAndQuery,'low',JSON.stringify(row));
 for(const row of d.tables?.['404']?.rows||[])push('broken-link','Enlace roto',row.url,'high',`HTTP ${row.statusCode}; encontrado en ${row.sourceUrl||row.sourceUqId||''}`);
 const errors=results.filter(r=>Number(r.status)>=500);
 for(const row of results)if(Number(row.status)>=500)push('server-error','Error de servidor observado',row.url,'high',`HTTP ${row.status}`,'availability',errors.length>=2);
 for(const item of d.summary?.items||[]) {
  if(item.status==='OK'||item.status==='NOTICE'||item.aplCode==='skipped')continue;
  const code=item.aplCode||'siteone';if(issues.some(i=>i.code===code))continue;
  const category=/seo|title|description|canonical|h1|heading|redirect|404|index/i.test(code)?'seo':/ssl|tls|certificate|security/i.test(code)?'security':'best-practices';
  push(code,clean(item.text),base,item.status==='CRITICAL'?'high':'medium','',category,/ssl-certificate-(trusted|valid)/.test(code));
 }
 const skipped=d.tables?.['skipped']?.rows||[];const skipSummary=d.tables?.['skipped-summary']?.rows||[];
 const omitted=skipSummary.filter(r=>/limit|max|depth/i.test(r.reason||'')).reduce((s,r)=>s+Number(r.count||0),0);
 const blocked=skipSummary.filter(r=>/robot|denied|forbidden/i.test(r.reason||'')).reduce((s,r)=>s+Number(r.count||0),0);
 const html=results.filter(r=>Number(r.type)===1).length||rows.length;
 const scores=Object.fromEntries((d.qualityScores?.categories||[]).map(c=>[c.code,Number(c.score)]));
 return {scores,score_scale:10,issues:issues.slice(0,200),coverage:{html_pages:html,rendered_pages:rendered?html:0,discovered:Number(d.stats?.totalUrls||results.length)+skipped.length,visited:results.length,blocked,omitted,limit_reached:results.length>=limit||omitted>0},pages:rows.slice(0,100).map(r=>({url:urlFor(r.urlPathAndQuery,base),title:clean(r.title),description:clean(r.description),h1:clean(r.h1),indexability:clean(r.indexing),canonical:clean(r.canonical||r.canonicalUrl)})),environment:{user_agent:d.crawler?.finalUserAgent,engine:d.crawler?.version,executed_at:d.crawler?.executedAt,rendering:rendered?'chromium':'html'},warnings:[]};
}
export function supplementary(html,url) {
 const $=cheerio.load(html);const issues=[];
 const push=(code,title,severity='medium',details='')=>issues.push({code,title,severity,details:clean(details),url,category:'seo',source:'Radar - validación complementaria'});
 $('link[rel="alternate"][hreflang]').each((_i,e)=>{const lang=$(e).attr('hreflang');if(lang==='x-default')return;try{new Intl.Locale(lang);}catch{push('hreflang-invalid','Etiqueta hreflang no válida','medium',lang);}});
 $('script[type="application/ld+json"]').each((_i,e)=>{try{JSON.parse($(e).text());}catch{push('jsonld-invalid','Datos estructurados JSON-LD no válidos');}});
 $('link[rel="canonical"]').each((_i,e)=>{try{const c=new URL($(e).attr('href'),url);if(c.protocol!=='https:'&&c.protocol!=='http:')push('canonical-invalid','Canonical no válido');}catch{push('canonical-invalid','Canonical no válido');}});
 return issues;
}
export function mergeRendered(plain,rendered){
 const seen=new Set((rendered.pages||[]).map(p=>p.url));
 return {...plain,issues:[...plain.issues.filter(i=>!seen.has(i.url)),...rendered.issues].slice(0,200),pages:[...(plain.pages||[]).filter(p=>!seen.has(p.url)),...(rendered.pages||[])].slice(0,100),coverage:{...plain.coverage,rendered_pages:rendered.coverage.html_pages},rendered_environment:rendered.environment,warnings:[...(plain.warnings||[]),...(rendered.warnings||[])]};
}
