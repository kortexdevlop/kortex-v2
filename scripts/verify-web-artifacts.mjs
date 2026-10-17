// Build-only checks: never execute code from the downloaded artifact.
import {readFileSync,writeFileSync,mkdirSync,readdirSync,lstatSync,cpSync} from 'node:fs';
import {resolve,join,relative,isAbsolute,sep} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const root=resolve(import.meta.dirname,'..');
const [directory,commit]=process.argv.slice(2);
if(!directory||!/^([a-f0-9]{40})$/.test(commit??''))throw Error('Expected artifact directory and full source commit');
const source=resolve(directory);
function command(program,args){const r=spawnSync(program,args,{cwd:root,encoding:'utf8'});if(r.status!==0)throw Error(program==='git'&&args[0]==='diff'?'Compiled sources differ: build a new release web artifact':`${program} failed: ${r.stderr}`);return r.stdout.trim();}
// Site integration may change independently; every compiled source/configuration must match.
command('git',['diff','--exit-code',commit,'--','Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo','crates','tools','apps','xtask','design','registry','brand']);
const tools=JSON.parse(readFileSync(join(root,'registry/index.json'),'utf8')).tools.map(({tool})=>tool.id);
const apps=JSON.parse(readFileSync(join(root,'registry/apps.json'),'utf8')).apps.filter(({app})=>['beta','stable'].includes(app.status)&&app.platforms.includes('web')).map(({app})=>app.id);
if(tools.length!==38)throw Error('The frozen catalogue must contain 38 tools');
mkdirSync(join(root,'.kortex/verification'),{recursive:true});
const verifier=join(root,'.kortex/verification/web-tree');
command('rustc',[join(root,'scripts/verify-web-tree.rs'),'-o',verifier]);
function files(base){if(!lstatSync(base).isDirectory()||lstatSync(base).isSymbolicLink())throw Error('Invalid bundle directory');const result={};function visit(dir){for(const entry of readdirSync(dir,{withFileTypes:true})){const path=join(dir,entry.name),stat=lstatSync(path);if(stat.isSymbolicLink())throw Error('Symlink in web artifact');if(stat.isDirectory())visit(path);else if(stat.isFile())result[relative(base,path).split(sep).join('/')]=createHash('sha256').update(readFileSync(path)).digest('hex');else throw Error('Non-regular web artifact');}}visit(base);return result;}
function exactIds(base,expected){const ids=readdirSync(base).sort();if(JSON.stringify(ids)!==JSON.stringify([...expected].sort()))throw Error(`Unexpected inventory: ${base}`);}
exactIds(join(source,'dist/run'),tools);exactIds(join(source,'dist/apps'),apps);
const proof={schema:1,commit,tools:{},apps:{}};
for(const [kind,ids] of [['tools',tools],['apps',apps]])for(const id of ids){
 if(!/^[a-z0-9.-]+$/.test(id))throw Error('Invalid catalogue id');
 const base=join(source,'dist',kind==='tools'?'run':'apps',id);
 const receipt=JSON.parse(readFileSync(join(source,'.kortex',kind==='tools'?`build/${id}/build-receipt.json`:`app-receipts/${id}-web.json`),'utf8'));
 if(command(verifier,[base])!==receipt.output)throw Error(`Receipt mismatch: ${id}`);
 const hashes=files(base);
 if(!hashes['index.html']||(kind==='tools'&&!hashes['sw.js']))throw Error(`Missing entry point: ${id}`);
 if(kind==='apps'){
  if(receipt.target!=='web'||!Object.keys(receipt.files??{}).length)throw Error(`Invalid app receipt: ${id}`);
  for(const [file,hash] of Object.entries(receipt.files)){
   const path=relative(base,resolve(base,file));if(isAbsolute(path)||path==='..'||path.startsWith('..'+sep)||hashes[file]!==hash)throw Error(`App manifest mismatch: ${id}`);
  }
 }
 proof[kind][id]={receipt,files:hashes};
}
// Copy only the verified bundles and receipts, never arbitrary files from the artifact root.
for(const name of ['run','apps'])cpSync(join(source,'dist',name),join(root,'dist',name),{recursive:true});
for(const id of tools){const dest=join(root,'.kortex/build',id);mkdirSync(dest,{recursive:true});cpSync(join(source,'.kortex/build',id,'build-receipt.json'),join(dest,'build-receipt.json'));}
mkdirSync(join(root,'.kortex/app-receipts'),{recursive:true});
for(const id of apps)cpSync(join(source,'.kortex/app-receipts',`${id}-web.json`),join(root,'.kortex/app-receipts',`${id}-web.json`));
writeFileSync(join(root,'.kortex/verification/web-artifacts.json'),JSON.stringify(proof,null,2)+'\n');
console.log(`Verified ${tools.length} tools and ${apps.length} apps from ${commit}`);
