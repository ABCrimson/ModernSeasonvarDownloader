const fs=require("fs"),vm=require("vm");const src=fs.readFileSync(process.argv[2],"utf8");
let out=null;vm.runInNewContext(src,{eval:(s)=>{out=s;},String,RegExp,parseInt});
fs.writeFileSync(process.argv[3],out);console.log("unpacked bytes",out.length);
