//! Compile-time route discovery and validation. Only routes under document_root are public.
use std::{fs,path::{Path,PathBuf}};
use crate::{build_config::{Routing,RouteRule},compiler,modules,storage::Storage};

fn uri_for(root:&Path,file:&Path)->Result<String,String>{
 let rel=file.strip_prefix(root).map_err(|e|e.to_string())?;
 Ok(format!("/{}",rel.to_string_lossy().replace('\\',"/")))
}
fn scan(root:&Path,sub:&Path,deny:&[String],entries:&mut Vec<(String,PathBuf)>)->Result<(),String>{
 for e in fs::read_dir(sub).map_err(|e|format!("{}: {e}",sub.display()))? {
  let e=e.map_err(|e|e.to_string())?;let ft=e.file_type().map_err(|e|e.to_string())?;
  if ft.is_symlink(){continue;}
  let name=e.file_name().to_string_lossy().to_string();if name.starts_with('.')||["target","build","dist","node_modules"].contains(&name.as_str()){continue;}
  let path=e.path();let url=uri_for(root,&path)?;
  if is_forbidden(&url,deny){continue;}
  if ft.is_dir(){scan(root,&path,deny,entries)?;}
  else if ft.is_file() && matches!(path.extension().and_then(|x|x.to_str()),Some("php"|"phx")) {entries.push((url,path));}
 }
 Ok(())
}
fn pattern_match(pattern:&str,path:&str)->bool{
 if let Some(base)=pattern.strip_suffix("/**"){return path==base||path.starts_with(&format!("{base}/"));}
 if pattern==path{return true;}
 let a:Vec<_>=pattern.split('/').collect();let b:Vec<_>=path.split('/').collect();
 a.len()==b.len()&&a.iter().zip(b.iter()).all(|(x,y)|x==y||x.starts_with(':')&&!y.is_empty())
}
fn is_forbidden(url:&str,deny:&[String])->bool{
 // Always block internal config, dotfiles and secrets even if a developer supplies an empty deny list.
 if url.split('/').any(|s|s.starts_with('.')&& !s.is_empty()){return true;}
 let last=url.rsplit('/').next().unwrap_or("");
 if ["config.json","phix.json","routes.json","composer.json","composer.lock"].contains(&last) {return true;}
 if [".env",".key",".pem",".p12"].iter().any(|suffix|last.ends_with(suffix)){return true;}
 deny.iter().any(|d|pattern_match(d,url))
}
fn quoted(s:&str)->String{format!("{s:?}")}
fn emit_rule(rule:&RouteRule)->String{format!("({}, {}, {}),",quoted(&rule.from),quoted(&rule.to),rule.status)}
pub fn generate(project:Option<&Path>,routing:&Routing,storage:&Storage,entry:&Path)->Result<String,String>{
 let mut entries=Vec::new();
 if let Some(project)=project{
   let root=fs::canonicalize(project).map_err(|e|e.to_string())?;
   let doc=fs::canonicalize(project.join(&routing.document_root)).map_err(|e|format!("routing.document_root: {e}"))?;
   if !doc.starts_with(&root)||!doc.is_dir(){return Err("document_root must be a directory within the project".into());}
   let mut denies=routing.deny.clone();
   if let Some(dir)=&storage.uploads {
    let upload_path=project.join(dir);
    // If storage lies inside public docroot, it must never become a public route.
    if let Ok(relative)=upload_path.strip_prefix(project.join(&routing.document_root)) {
     if !relative.as_os_str().is_empty(){
      let prefix=format!("/{}",relative.to_string_lossy().replace('\\',"/"));
      denies.push(format!("{prefix}/**"));
     }
    }
   }
   scan(&doc,&doc,&denies,&mut entries)?;
 }else{entries.push(("/index.php".into(),entry.to_owned()));}
 entries.sort_by(|a,b|a.0.cmp(&b.0));
 if entries.len()>256{return Err("too many public PHP entrypoints (max 256)".into());}
 let contains=|s:&str|entries.iter().any(|(url,_)|url==s);
 if let Some(f)=&routing.fallback{if !contains(f)||is_forbidden(f,&routing.deny){return Err(format!("fallback references a missing or denied PHP entrypoint: {f}"));}}
 for rule in &routing.rewrites{
  let target=rule.to.split('?').next().unwrap();
  if !contains(target)||is_forbidden(target,&routing.deny){return Err(format!("rewrite target not a public compiled PHP entrypoint: {target}"));}
 }
 let mut out=String::new();
 for (i,(_,path)) in entries.iter().enumerate(){
  let tokens=modules::expand(path)?;
  let code=compiler::compile(&tokens).map_err(|e|format!("{}: {e}",path.display()))?;
  out.push_str(&format!("mod phix_route_{i} {{ use super::*; {code} pub(super) fn run(ctx: &mut RequestContext) {{ phix_app_request(ctx); }} }}\n"));
 }
 out.push_str("static PHIX_PUBLIC_ROUTES: &[&str] = &[\n");for (name,_) in &entries{out.push_str(&format!("{},\n",quoted(name)));}out.push_str("];\n");
 out.push_str("fn phix_route_run(index:usize,ctx:&mut RequestContext){match index{\n");for i in 0..entries.len(){out.push_str(&format!("{i}=>phix_route_{i}::run(ctx),\n"));}out.push_str("_=>{},}}\n");
 out.push_str(&format!("static PHIX_DOCUMENT_ROOT: &str = {};\n",quoted(if routing.document_root=="."{""}else{routing.document_root.trim_end_matches('/')})));
 out.push_str("static PHIX_DENY: &[&str] = &[\n");let mut effective_deny=routing.deny.clone();
 if let (Some(p),Some(folder))=(project,&storage.uploads){if let Ok(rel)=p.join(folder).strip_prefix(p.join(&routing.document_root)){if !rel.as_os_str().is_empty(){effective_deny.push(format!("/{}/**",rel.to_string_lossy().replace('\\',"/")));}}}
 for x in &effective_deny{out.push_str(&format!("{},\n",quoted(x)));}out.push_str("];\n");
 for (key,rules) in [("PHIX_REWRITES",&routing.rewrites),("PHIX_REDIRECTS",&routing.redirects)]{
  out.push_str(&format!("static {key}: &[(&str,&str,u16)] = &[\n"));for rule in rules{out.push_str(&emit_rule(rule));out.push('\n');}out.push_str("];\n");
 }
 out.push_str(&format!("static PHIX_FALLBACK: Option<&str> = {};\n",routing.fallback.as_deref().map(|s|format!("Some({})",quoted(s))).unwrap_or("None".into())));
 Ok(out)
}
