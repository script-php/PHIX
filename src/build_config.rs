//! Build metadata and safe runtime configuration deployment.
//! Never overwrite an existing config.json beside the output executable.
use std::{fs, io::{self,Write}, path::{Component,Path,PathBuf}};
use crate::json::{Json,JsonParser};
use crate::storage::Storage;

#[derive(Clone,Debug)]
pub struct RouteRule {pub from:String,pub to:String,pub status:u16}
#[derive(Clone,Debug)]
pub struct Routing {pub document_root:String,pub deny:Vec<String>,pub rewrites:Vec<RouteRule>,pub redirects:Vec<RouteRule>,pub fallback:Option<String>}
impl Default for Routing {fn default()->Self{Self{document_root:".".into(),deny:vec!["/includes/**".into(),"/classes/**".into(),"/vendor/**".into(),"/storage/**".into(),"/config/**".into(),"/tests/**".into()],rewrites:vec![],redirects:vec![],fallback:None}}}
pub struct BuildConfig { entry: Option<String>,pub routing:Routing,pub storage:Storage }
fn object_fields(v:Json,what:&str)->Result<Vec<(String,Json)>,String>{if let Json::Object(o)=v{Ok(o)}else{Err(format!("{what} must be an object"))}}
fn text(v:Json,what:&str)->Result<String,String>{if let Json::Text(s)=v{Ok(s)}else{Err(format!("{what} must be a string"))}}
fn valid_rel_path(path:&str)->bool{let p=Path::new(path);!path.is_empty() && p.components().all(|c|matches!(c,Component::Normal(_))) && !path.contains('\\')}
fn valid_url_path(s:&str)->bool{s.starts_with('/')&&!s.contains('\\')&&!s.contains('%')&&!s.contains('?')&&!s.contains('#')&&!s.split('/').any(|x|x==".."||x==".") && !s.bytes().any(|b|b<32)}
fn valid_pattern(s:&str)->bool{valid_url_path(s) && s.split('/').all(|v| !v.contains('*') || v=="**")}
fn rules(value:Json,redirect:bool)->Result<Vec<RouteRule>,String>{
 let Json::Array(items)=value else{return Err("routing rule list must be an array".into())};
 if items.len()>256{return Err("too many routing rules (max 256)".into())}
 let mut result=Vec::new();
 for item in items{let mut from=None;let mut to=None;let mut status=if redirect{302}else{0};
  for (key,value) in object_fields(item,"routing rule")? {match key.as_str(){"from"=>from=Some(text(value,"rule.from")?),"to"=>to=Some(text(value,"rule.to")?),"status" if redirect=>{status=match value{Json::Int(301|302|303|307|308)=>if let Json::Int(n)=value{n as u16}else{unreachable!()},_=>return Err("redirect status must be 301, 302, 303, 307 or 308".into())}},_=>return Err(format!("unknown routing rule field: {key}"))}}
  let from=from.ok_or("routing rule missing from")?;let to=to.ok_or("routing rule missing to")?;
  if !valid_pattern(&from)||from.contains("**"){return Err(format!("invalid routing rule from: {from}"))}
  if !to.starts_with('/')||to.starts_with("//")||to.contains('\\')||to.contains('#')||to.contains('\r')||to.contains('\n')||to.split('?').next().unwrap_or("").split('/').any(|v|v==".."||v=="."){return Err(format!("invalid routing target: {to}"))}
  if !redirect && !matches!(to.split('?').next().unwrap_or("").rsplit('.').next(),Some("php"|"phx")){return Err("rewrite target must be a .php/.phx entrypoint".into())}
  result.push(RouteRule{from,to,status});
 }
 Ok(result)
}
fn parse_routing(v:Json)->Result<Routing,String>{let mut r=Routing::default();for (key,value) in object_fields(v,"routing")?{
 match key.as_str(){
 "document_root"=>{r.document_root=text(value,"routing.document_root")?;if r.document_root!="."&&!valid_rel_path(&r.document_root){return Err("routing.document_root must stay within the project".into())}},
 "deny"=>{let Json::Array(items)=value else{return Err("routing.deny must be an array".into())};if items.len()>256{return Err("too many denied paths".into())}r.deny=Vec::new();for item in items{let path=text(item,"routing.deny item")?;if !valid_pattern(&path){return Err(format!("invalid denied path: {path}"))}r.deny.push(path);}},
 "rewrites"=>r.rewrites=rules(value,false)?,
 "redirects"=>r.redirects=rules(value,true)?,
 "fallback"=>{let path=text(value,"routing.fallback")?;if !valid_url_path(&path)||!matches!(path.rsplit('.').next(),Some("php"|"phx")){return Err("routing.fallback must reference a .php/.phx URL".into())}r.fallback=Some(path)},
 _=>return Err(format!("unknown routing field: {key}"))}
 }
 Ok(r)
}

impl BuildConfig {
 pub fn load(project:Option<&Path>)->Result<Self,String>{
  let Some(project)=project else{return Ok(Self{entry:None,routing:Routing::default(),storage:Storage::default()})};
  let path=project.join("phix.json");
  let text=match fs::read_to_string(&path){Ok(s)=>s,Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(Self{entry:None,routing:Routing::default(),storage:Storage::default()}),Err(e)=>return Err(format!("{}: {e}",path.display()))};
  if text.len()>1024*1024{return Err("phix.json exceeds 1 MiB".into());}
  let json=JsonParser::parse(&text).map_err(|e|format!("{}: {e}",path.display()))?;
  let Json::Object(fields)=json else{return Err("phix.json root must be a JSON object".into())};
  let mut entry=None;let mut routing=Routing::default();let mut storage=Storage::default();
  for (key,value) in fields {
   match (key.as_str(),value) {
    ("entry", Json::Text(s))=>entry=Some(s),
    ("entry",_)=>return Err("phix.json entry must be a string".into()),
    ("routing",v)=>routing=parse_routing(v)?,
    ("storage",v)=>storage=Storage::parse(v)?,
    _=>return Err(format!("unknown phix.json build key: {key}")),
   }
  }
  Ok(Self{entry,routing,storage})
 }
 pub fn entrypoint(&self,project:Option<&Path>,fallback:PathBuf)->Result<PathBuf,String>{
  let Some(ref entry)=self.entry else {return Ok(fallback)};
  let root=project.ok_or("phix.json entry requires a project directory")?;
  let rel=Path::new(entry);
  if rel.as_os_str().is_empty()||!rel.components().all(|c|matches!(c,Component::Normal(_))){return Err("phix.json entry must be a relative path inside the project".into());}
  if !matches!(rel.extension().and_then(|x|x.to_str()),Some("php"|"phx")){return Err("phix.json entry must end in .php or .phx".into())}
  let canonical_root=fs::canonicalize(root).map_err(|e|format!("project directory: {e}"))?;
  let full=fs::canonicalize(root.join(rel)).map_err(|e|format!("phix.json entry '{entry}': {e}"))?;
  if !full.starts_with(&canonical_root)||!full.is_file(){return Err("phix.json entry must be a source file inside the project".into());}
  Ok(full)
 }
}

pub fn deploy_config(project:Option<&Path>,output:&Path)->Result<String,String>{
 let parent=output.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
 let dest=parent.join("config.json");
 // Open with create_new: never replace an existing file, even under concurrent builds.
 let mut target=match fs::OpenOptions::new().write(true).create_new(true).open(&dest){
  Ok(f)=>f,
  Err(e) if e.kind()==io::ErrorKind::AlreadyExists=>return Ok(format!("{} (kept existing)",dest.display())),
  Err(e)=>return Err(format!("cannot create {}: {e}",dest.display()))
 };
 let source=project.map(|p|p.join("config.json"));
 let result=(||->Result<String,String>{
  let contents=if let Some(src)=source.as_ref().filter(|p|p.exists()){
   if fs::symlink_metadata(src).map(|m|m.file_type().is_symlink()).unwrap_or(false){return Err(format!("refusing symlinked project config: {}",src.display()))}
   let txt=fs::read_to_string(src).map_err(|e|format!("{}: {e}",src.display()))?;
   if txt.len()>1024*1024{return Err("project config.json exceeds 1 MiB".into())}
   let parsed=JsonParser::parse(&txt).map_err(|e|format!("{}: {e}",src.display()))?;
   validate_runtime_json(&parsed)?;
   txt
  }else{String::from("{\n  \"runtime\": {\n    \"port\": 9510,\n    \"bind\": \"127.0.0.1\"\n  }\n}\n")};
  target.write_all(contents.as_bytes()).map_err(|e|format!("{}: {e}",dest.display()))?;
  target.sync_all().map_err(|e|format!("{}: {e}",dest.display()))?;
  Ok(format!("{} (created)",dest.display()))
 })();
 if result.is_err(){drop(target);let _=fs::remove_file(&dest);}
 result
}
fn validate_runtime_json(value:&Json)->Result<(),String>{
 let Json::Object(_)=value else{return Err("config.json root must be a JSON object".into())};
 let runtime=match value.get("runtime"){
  None=>return Ok(()),
  Some(Json::Object(_))=>value.get("runtime").unwrap(),
  _=>return Err("config.json runtime must be an object".into())
 };
 match runtime.get("port"){
  None|Some(Json::Int(1..=65535))=>{},
  _=>return Err("config.json runtime.port must be an integer 1..65535".into())
 }
 match runtime.get("bind"){
  None=>{},
  Some(Json::Text(ip)) if ip.parse::<std::net::Ipv4Addr>().is_ok()=>{},
  _=>return Err("config.json runtime.bind must be an IPv4 address".into())
 }
 for (name,min,max) in [("max_request_body",1,67108864),("max_header_bytes",1024,32768),("max_concurrent_requests",1,256),("io_timeout_seconds",1,30)]{
  match runtime.get(name){None=>{},Some(Json::Int(n)) if *n>=min && *n<=max=>{},_=>return Err(format!("config.json runtime.{name} must be an integer in {min}..{max}"))}
 }
 Ok(())
}
