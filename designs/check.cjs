const fs=require('fs'),path=require('path'),vm=require('vm');
const root=process.argv[2]||path.dirname(__dirname);
const lum=h=>{const c=h.replace('#','');const n=c.length===3?c.split('').map(x=>x+x).join(''):c;const a=[0,2,4].map(i=>parseInt(n.slice(i,i+2),16)/255).map(x=>x<=.04045?x/12.92:((x+.055)/1.055)**2.4);return a[0]*.2126+a[1]*.7152+a[2]*.0722};
const ratio=(a,b)=>(Math.max(lum(a),lum(b))+.05)/(Math.min(lum(a),lum(b))+.05);
let count=0,min=99;
for(const screen of ['main-window','room','chat'])for(let v=1;v<=3;v++){
const html=fs.readFileSync(path.join(root,'designs',screen,`v${v}.html`),'utf8');
const assert=(x,m)=>{if(!x)throw Error(`${screen}/v${v}: ${m}`)};
const ids=[...html.matchAll(/data-id="([^"]+)"/g)].map(x=>x[1]);assert(new Set(ids).size===ids.length,'duplicate data-id');
assert((html.match(/<h1/g)||[]).length===1,'one h1');assert(!/src="https?:|fetch\(/.test(html),'external call');
if(screen==='room')assert((html.match(/data-id="message-/g)||[]).length>=200,'200-message history');
new vm.Script(html.match(/<script>([\s\S]*?)<\/script>/)[1]);
const declarations=selector=>{const escaped=selector.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');const block=html.match(new RegExp(escaped+'\\{([^}]+)\\}'));return Object.fromEntries([...(block?.[1]||'').matchAll(/--([\w-]+):\s*(#[0-9a-f]+)/g)].map(x=>[x[1],x[2]]))};
const light={...declarations(':root'),...declarations('body.v'+v)};
const dark={...light,...declarations('html.dark'),...declarations('html.dark body.v'+v)};
light.on=light['on-accent'];dark.on=dark['on-accent'];
for(const t of [light,dark]){for(const fg of ['ink','muted'])for(const bg of ['canvas','surface','soft','selected']){const r=ratio(t[fg],t[bg]);assert(r>=4.5,`${fg}/${bg} contrast ${r}`);min=Math.min(min,r)}assert(ratio(t.on,t.accent)>=4.5,'primary contrast');assert(ratio(t.line,t.surface)>=3,'control outline contrast');}
count++;
}
console.log(`${count} pages: IDs, script syntax, local-only content, history, and role contrast passed. Minimum text contrast ${min.toFixed(2)}:1.`);
