const fs=require('fs'),path=require('path'),vm=require('vm');
for(let v=4;v<=6;v++)for(const s of ['main-window','room','chat']){
const h=fs.readFileSync(path.join(__dirname,s,'v'+v+'.html'),'utf8');
const ids=[...h.matchAll(/data-id="([^"]+)"/g)].map(x=>x[1]);
const assert=(b,m)=>{if(!b)throw Error(s+'/v'+v+': '+m)};
assert(ids.length===new Set(ids).size,'duplicate design ID');
assert((h.match(/<h1>/g)||[]).length===1,'one title');
assert(!/fetch\(|https?:/.test(h),'local-only');
assert(!h.includes('Sample workspace'),'prototype chrome');
assert(h.includes('html.dark{'),'dark theme');
if(s!=='main-window'){assert((h.match(/data-id="message-/g)||[]).length===18,'18 messages');assert(h.includes('data-id="handoff"'),'hand-off');assert(h.includes('data-id="side-job"'),'side job');assert(h.includes('data-id="tool-failed"'),'failed tool');}
new vm.Script(h.match(/<script>([\s\S]*?)<\/script>/)[1]);
}console.log('9 pages passed: titles, IDs, scripts, themes, local content, 18-message transcripts and work events.');
