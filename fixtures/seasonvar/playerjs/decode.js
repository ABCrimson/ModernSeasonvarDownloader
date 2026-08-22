const fs=require("fs");
// worker's documented algorithm: strip "#2", remove FIRST occurrence of "//"+btoa(bk) for bk4..bk0 (only bk0="ololo"), then utf8 atob
function b1(s){return Buffer.from(s,"utf8").toString("base64");}
function decodeWorker(x){ if(!x.startsWith("#2")) return {err:"no #2 prefix"}; let a=x.substr(2); a=a.replace("//"+b1("ololo"),""); try{ const u=Buffer.from(a,"base64").toString("utf8"); return {url:u}; }catch(e){return {err:String(e)}} }
const re=/^\/\/[A-Za-z0-9.-]+\/[^\s"']+\.mp4$/;
let ok=0,fail=0,out=[],notes={};
for(const f of process.argv.slice(2)){
  const arr=JSON.parse(fs.readFileSync(f,"utf8"));
  const items=[]; (function walk(a){for(const it of a){ if(it.folder) walk(it.folder); else items.push(it);} })(arr);
  for(const it of items){
    const keys=Object.keys(it).join(",");
    const cnt=it.file.split("//b2xvbG8=").length-1;
    const grid=it.file.includes("//Z3JpZA==");
    const d=decodeWorker(it.file);
    const good=d.url && re.test(d.url) && !/[^\x20-\x7e]/.test(d.url);
    if(good) ok++; else fail++;
    out.push({f:f.split("/").pop(),id:it.id,title:it.title,keys,junk:cnt,grid,sub:it.subtitle,url:d.url||d.err,good});
  }
}
fs.writeFileSync(process.argv[2].replace(/[^\/]+$/,"")+"decoded-verify.json",JSON.stringify(out,null,1));
console.log("ok",ok,"fail",fail,"total",ok+fail);
console.log("junk counts:",JSON.stringify(out.reduce((m,o)=>(m[o.junk]=(m[o.junk]||0)+1,m),{})),"grid:",out.filter(o=>o.grid).length);
console.log("keysets:",[...new Set(out.map(o=>o.keys))].join(" | "));
console.log("hosts:",[...new Set(out.map(o=>(o.url.match(/^\/\/([^\/]+)/)||[])[1]))].join(","));
console.log("prefixes:",[...new Set(out.map(o=>(o.url.match(/fi2lm\/[0-9a-f]{32}\/(\w+_)/)||[])[1]))].join(","));
out.filter(o=>!o.good).forEach(o=>console.log("FAIL",JSON.stringify(o)));
out.slice(0,4).forEach(o=>console.log(o.f,o.id,"|",o.title,"|",o.url));
