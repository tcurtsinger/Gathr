const fs=require('fs'),path=require('path'),vm=require('vm'),assert=require('assert/strict');
const pages=[['chat',13],['single-chat',11],['new-chat',11],['start-chat',12]];
for(const[d,v]of pages){const file=path.join(__dirname,d,'v'+v+'.html'),h=fs.readFileSync(file,'utf8');const ids=[...h.matchAll(/data-id="([^"]+)"/g)].map(x=>x[1]);assert.equal(new Set(ids).size,ids.length,'unique data IDs');assert.equal((h.match(/<h1/g)||[]).length,1);assert(!/fetch\(|Sample workspace|>↑<|>＋<|<span class="path">Gathr/.test(h),'local content and polished icons');assert(h.includes('../tokens.css')&&h.includes('../components.css'),'shared styles');if(d==='chat'||d==='single-chat')assert.equal((h.match(/data-id="message-/g)||[]).length,42);for(const m of h.matchAll(/<script>([\s\S]*?)<\/script>/g))new vm.Script(m[1]);for(const m of h.matchAll(/(?:href|location.href)=['"]([^'"#]+\.html)/g))assert(fs.existsSync(path.resolve(path.dirname(file),m[1])),m[1]);}
assert(fs.readFileSync(path.join(__dirname,'components.css'),'utf8').includes('.choose .agentcheck:checked'),'picker styling');console.log('Four selected pages: IDs, scripts, links, shared styles, fixtures and polish passed.');

assert(fs.readFileSync(path.join(__dirname,'components.css'),'utf8').includes('.chatrow[hidden]{display:none}'),'search hides flex rows');

new vm.Script(fs.readFileSync(path.join(__dirname,'navigation.js'),'utf8')));
assert(fs.readFileSync(path.join(__dirname,'components.css'),'utf8').includes('background:var(--backdrop)'),'shared backdrop');
for(const[d,v]of pages){const h=fs.readFileSync(path.join(__dirname,d,'v'+v+'.html'),'utf8');assert(h.includes('src="../navigation.js"')&&h.includes('data-id="sidebar-reveal"'),'shared navigation reveal');}
