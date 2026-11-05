import test from 'node:test';
import assert from 'node:assert/strict';
import {publicAddress,destination} from '../public-proxy.mjs';
import {lighthouseResult,siteoneResult,supplementary,mergeRendered} from '../normalize.mjs';
test('rechaza red privada, metadatos, loopback y variantes IPv6',async()=>{
 for(const ip of ['127.0.0.1','10.0.0.1','172.16.1.1','192.168.1.1','169.254.169.254','100.64.0.1','::1','fc00::1','fe80::1','::ffff:127.0.0.1'])assert.equal(publicAddress(ip),false,ip);
 for(const url of ['http://127.1','http://2130706433','http://[::ffff:127.0.0.1]','http://localhost','http://a.local','https://user:secret@example.com','http://8.8.8.8:3000','file:///etc/passwd'])await assert.rejects(destination(url));
 assert.equal(publicAddress('8.8.8.8'),true);
});
test('Lighthouse conserva perfil y métricas reales; no inventa INP',()=>{
 const mobile=lighthouseResult({lighthouseVersion:'13.6.0',configSettings:{formFactor:'mobile'},categories:{performance:{score:.51}},audits:{'largest-contentful-paint':{numericValue:4400,displayValue:'4.4 s'}}});
 const desktop=lighthouseResult({configSettings:{formFactor:'desktop'},audits:{}});
 assert.equal(mobile.scores.performance,51);assert.equal(mobile.metrics.find(m=>m.label==='LCP').value,4400);assert.equal(mobile.environment.profile,'mobile');assert.equal(desktop.environment.profile,'desktop');assert.equal(mobile.metrics.some(m=>m.label==='INP'),false);
});
test('SiteOne identifica fallos por URL y conserva límites; JS sustituye evidencia HTML',()=>{
 const plain=siteoneResult({results:[{url:'https://example.com/a',type:1,status:200}],tables:{seo:{rows:[{urlPathAndQuery:'/a',title:'',description:'',h1:''}]}},summary:{items:[{aplCode:'skipped',status:'CRITICAL',text:'Skipped URLs'}]}},'https://example.com',1);
 assert.equal(plain.coverage.limit_reached,true);assert.ok(plain.issues.some(i=>i.code==='title-missing'&&i.url==='https://example.com/a'));assert.equal(plain.issues.some(i=>i.code==='skipped'),false);
 const rendered=siteoneResult({results:[{type:1,status:200}],tables:{seo:{rows:[{urlPathAndQuery:'/a',title:'Título real',description:'Descripción real',h1:'H1 real'}]}}},'https://example.com',5,true);
 const merged=mergeRendered(plain,rendered);assert.equal(merged.coverage.rendered_pages,1);assert.equal(merged.issues.some(i=>i.code==='title-missing'),false);assert.equal(merged.pages[0].title,'Título real');
});
test('validaciones complementarias no confunden copyright ni constructor con SEO',()=>{
 const issues=supplementary('<meta name="generator" content="Wix"><p>© 2018</p><script type="application/ld+json">{mal</script><link rel="alternate" hreflang="es_XX"/>','https://example.com');
 assert.ok(issues.some(i=>i.code==='jsonld-invalid'));assert.ok(issues.some(i=>i.code==='hreflang-invalid'));assert.equal(issues.length,2);
});
test('un fallo HTTP aislado no atribuye el problema al hosting',()=>{
 const r=siteoneResult({results:[{type:1,url:'https://example.com',status:503}]},'https://example.com');assert.equal(r.issues[0].hosting_evidence,false);
});
