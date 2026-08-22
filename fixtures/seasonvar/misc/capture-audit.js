const fs=require("fs");const path=require("path");
const R=process.env.R; const FX=path.join(R,"fixtures");
const UA="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/128 Safari/537.36";
const JUNK=["//b2xvbG8=","//Z3JpZA=="];
function decodeTok(tok){
  let t=tok; const notes=[];
  if(!t.startsWith("#2")) notes.push("NO_#2_PREFIX");
  else t=t.slice(2);
  for(const j of JUNK){ let c=0; while(t.includes(j)){t=t.replace(j,"");c++;} if(c) notes.push(j+"x"+c); }
  // Any remaining "//" inside the base64 body? (not valid b64 char) → unknown junk
  const m=t.match(/\/\/[A-Za-z0-9+\/=]*/g); if(m) notes.push("OTHER_JUNK:"+m.join("|"));
  let dec=""; try{dec=Buffer.from(t,"base64").toString("utf8");}catch(e){notes.push("B64ERR")}
  return {dec,notes};
}
async function get(url,opts={}){
  const r=await fetch(url,{headers:{"User-Agent":UA,...(opts.headers||{})},redirect:"manual"});
  const buf=Buffer.from(await r.arrayBuffer());
  return {status:r.status,headers:Object.fromEntries(r.headers.entries()),body:buf};
}
async function audit(url){
  const id=url.match(/serial-(\d+)/)[1];
  const pg=await get(url);
  fs.writeFileSync(path.join(FX,`serial-${id}.html`),pg.body);
  fs.writeFileSync(path.join(R,"raw",`serial-${id}.headers.json`),JSON.stringify(pg.headers,null,1));
  const html=pg.body.toString("utf8");
  const out={url,id,status:pg.status,setCookie:pg.headers["set-cookie"]||null};
  const d4=html.match(/var data4play\s*=\s*\{([\s\S]*?)\}/); out.data4play=d4?Object.fromEntries([...d4[1].matchAll(/'(\w+)':\s*'([^']*)'/g)].map(m=>[m[1],m[2]])):null;
  out.title=(html.match(/<title>([^<]*)<\/title>/)||[])[1];
  out.h1=(html.match(/<h1[^>]*>([\s\S]*?)<\/h1>/)||[])[1]?.replace(/\s+/g," ").trim();
  out.ogImage=(html.match(/property="og:image"\s+content="([^"]*)"/)||[])[1];
  out.seasonLinks=[...new Set([...html.matchAll(/href="(\/serial-\d+-[^"]*)"/g)].map(m=>m[1]))].filter(h=>h.includes(`/serial-${id}-`)||true).slice(0,0);
  // season nav: links inside pgs-seaslist
  const seas=html.match(/<(?:ul|div)[^>]*pgs-seaslist[\s\S]*?<\/(?:ul|div)>/); out.seasonNav=seas?[...seas[0].matchAll(/href="([^"]*)"[^>]*>([\s\S]*?)<\/a>/g)].map(m=>[m[1],m[2].replace(/<[^>]*>/g,"").trim()]):null;
  out.seasonNavRawSample=seas?seas[0].slice(0,300):null;
  out.sameSerialLinks=[...new Set([...html.matchAll(/href="(\/serial-\d+-[^"]*)"/g)].map(m=>m[1]))];
  const plInit=html.match(/var pl\s*=\s*(\{[^\n]*\});/); out.plInit=plInit?plInit[1]:null;
  const pls={}; if(plInit){for(const m of plInit[1].matchAll(/'(\d+)':\s*"([^"]*)"/g))pls[m[1]]=m[2];}
  for(const m of html.matchAll(/pl\[(\d+)\]\s*=\s*"([^"]*)"/g))pls[m[1]]=m[2];
  const trans={}; const ul=html.match(/<ul class="pgs-trans">([\s\S]*?)<\/ul>/);
  if(ul){for(const m of ul[1].matchAll(/<li data-click="translate" data-translate="(\d+)"([^>]*)>([^<]*)<\/li>/g))trans[m[1]]={name:m[3],attrs:m[2].trim()};}
  out.hasPgsTrans=!!ul; out.trans=trans; out.pls=pls;
  const ar=html.match(/var arEpisodes\s*=\s*(\{.*?\});\n/s); out.arEpisodes=ar?JSON.parse(ar[1]):null;
  out.playerInit=(html.match(/new Playerjs\(\{[^\n]*/)||[])[0];
  out.scripts=[...new Set([...html.matchAll(/<script[^>]*src="([^"]*)"/g)].map(m=>m[1]))];
  out.playlists={};
  for(const [tid,rel] of Object.entries(pls)){
    const purl="https://seasonvar.ru"+rel; const r=await get(purl);
    const fn=path.join(FX,`plist-${id}-${tid}.json`); fs.writeFileSync(fn,r.body);
    const txt=r.body.toString("utf8"); let arr=null; try{arr=JSON.parse(txt);}catch(e){}
    const p={url:purl,status:r.status,ct:r.headers["content-type"],len:r.body.length,isArray:Array.isArray(arr),count:arr?arr.length:null,fieldSets:{},subtitleNonEmpty:[],titleHtml:0,hosts:{},qualityTokens:{},alternates:0,decodeNotes:{},sample:[],prefixes:{},nonMatchingDec:[],idPattern:[]};
    if(arr){ for(const it of arr){
      const keys=Object.keys(it).sort().join(","); p.fieldSets[keys]=(p.fieldSets[keys]||0)+1;
      if(it.subtitle) p.subtitleNonEmpty.push(it.subtitle);
      if(/<|&[a-z]+;/.test(it.title)) p.titleHtml++;
      const {dec,notes}=decodeTok(it.file||""); for(const n of notes)p.decodeNotes[n]=(p.decodeNotes[n]||0)+1;
      if(dec.includes(" or ")) p.alternates++;
      for(const part of dec.split(" or ")){ const h=(part.match(/^\/\/([^\/]+)\//)||[])[1]; p.hosts[h]=(p.hosts[h]||0)+1;
        const q=(part.match(/\.(\w+)\.mp4$/)||[])[1]; p.qualityTokens[q]=(p.qualityTokens[q]||0)+1;
        const pm=part.match(/\/fi2lm\/[0-9a-f]{32}\/([^.]+?)_/); p.prefixes[pm?pm[1]:"?"]=(p.prefixes[pm?pm[1]:"?"]||0)+1;
        if(!/^\/\/data\d+-cdn\.11cdn\.org\/fi2lm\/[0-9a-f]{32}\/7f_.+\.s\d+e\d+\.\w+\.mp4$/.test(part)) p.nonMatchingDec.push(part);
      }
      if(p.sample.length<3) p.sample.push({title:it.title,file:it.file,dec,subtitle:it.subtitle,galabel:it.galabel,id:it.id,vars:it.vars});
      if(p.idPattern.length<3)p.idPattern.push(it.id);
    }}
    p.nonMatchingDec=p.nonMatchingDec.slice(0,5);
    out.playlists[tid]=p;
  }
  return out;
}
(async()=>{const res=[];for(const u of process.argv.slice(2)){try{res.push(await audit(u));console.error("done",u);}catch(e){res.push({url:u,error:String(e)});console.error("ERR",u,e);}}
fs.writeFileSync(path.join(R,"audit-"+Date.now()+".json"),JSON.stringify(res,null,1));
for(const r of res){console.log("==",r.url,r.status,"mark",r.data4play?.secureMark,"time",r.data4play?.time,"trans",JSON.stringify(Object.fromEntries(Object.entries(r.trans||{}).map(([k,v])=>[k,v.name]))),"pls",Object.keys(r.pls||{}).join(","),"cookie",r.setCookie);
 for(const [t,p] of Object.entries(r.playlists||{}))console.log("  pl",t,p.status,p.ct,"n=",p.count,"fields",JSON.stringify(p.fieldSets),"subs",p.subtitleNonEmpty.length,"titleHtml",p.titleHtml,"hosts",JSON.stringify(p.hosts),"q",JSON.stringify(p.qualityTokens),"alt",p.alternates,"notes",JSON.stringify(p.decodeNotes),"prefix",JSON.stringify(p.prefixes),"nonmatch",JSON.stringify(p.nonMatchingDec));}
})();
