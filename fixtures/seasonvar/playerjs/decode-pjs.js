const fs=require("fs"),vm=require("vm");const s=fs.readFileSync(process.argv[2],"utf8");
function grab(re){const m=s.match(re);if(!m)throw new Error("no match "+re);return m[0];}
const abc=grab(/var abc=[^;]+;/);
const dechar=grab(/(var dechar=function\([^)]*\)\{[^}]*\};|function dechar\([^)]*\)\{[^}]*\})/);
// salt: from "var salt={" to the matching close — take until "};var " after _ud
const si=s.indexOf("var salt={");const se=s.indexOf("};var ",s.indexOf("_ud:function",si));const salt=s.slice(si,se+2);
const pepper=grab(/var pepper=function\(s,n\)\{.*?\)\}\)\};/);
const sugar=grab(/var sugar=function\(x\)\{.*?return result\.substr\(0,result\.length-1\)\};/);
const decode=grab(/var decode=function\(x\)\{.*?return x\}\};/);
const y=grab(/y:'[^']*'/).slice(3,-1);
const fd2arg=grab(/function fd2\(x\)\{var a;eval\(decode\('#1[^']+'\)\)/).match(/'(#1[^']+)'/)[1];
const u=grab(/u:'#1[^']+'/).slice(3,-1);
const ctx={o:{y},console};
vm.createContext(ctx);
vm.runInContext(abc+dechar+salt+pepper+sugar+decode,ctx);
const fd2body=vm.runInContext("decode("+JSON.stringify(fd2arg)+")",ctx);
const ubody=vm.runInContext("decode("+JSON.stringify(u)+")",ctx);
fs.writeFileSync(process.argv[3],fd2body);fs.writeFileSync(process.argv[4],ubody);
console.log("y=",y);console.log("fd2 body:\n"+fd2body.slice(0,400));
const uo=JSON.parse(ubody);console.log("u keys",Object.keys(uo).length,"bk*:",JSON.stringify(Object.fromEntries(Object.entries(uo).filter(([k])=>/^bk/.test(k)))),"file3_separator in u:",uo.file3_separator);
