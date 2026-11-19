//! Build-time upload storage policy and non-destructive deployment.
//! An upload directory is explicitly opt-in. It is never embedded in the executable.
use std::{fs,io,path::{Component,Path}};
use crate::json::Json;

#[derive(Clone,Debug)]
pub struct Storage {pub uploads: Option<String>, pub max_file_size:usize}
impl Default for Storage {fn default()->Self{Self{uploads:None,max_file_size:10*1024*1024}}}
fn valid_rel(s:&str)->bool{
 !s.is_empty() && !s.contains('\\') && Path::new(s).components().all(|p|matches!(p,Component::Normal(_)))
 && !s.split('/').any(|x|x.starts_with('.')||x.is_empty())
}
impl Storage {
 pub fn parse(v:Json)->Result<Self,String>{
  let Json::Object(items)=v else{return Err("storage must be an object".into())};
  let mut p=Storage::default();
  for (key,value) in items {match (key.as_str(),value){
   ("uploads",Json::Text(path))=>{if !valid_rel(&path){return Err("storage.uploads must be a relative folder within the project".into())}p.uploads=Some(path)},
   ("max_file_size",Json::Int(n)) if (1..=32*1024*1024).contains(&n)=>p.max_file_size=n as usize,
   ("max_file_size",_)=>return Err("storage.max_file_size must be 1..33554432 bytes".into()),
   ("uploads",_)=>return Err("storage.uploads must be a string".into()),
   _=>return Err(format!("unknown storage field: {key}"))
  }}
  if p.uploads.is_none() && p.max_file_size!=10*1024*1024 {return Err("storage.max_file_size requires storage.uploads".into())}
  Ok(p)
 }
 pub fn emitted(&self)->String{
  let dir=match &self.uploads{Some(s)=>format!("Some({s:?})"),None=>"None".into()};
  format!("static PHIX_UPLOAD_DIR: Option<&str> = {dir};\nstatic PHIX_UPLOAD_MAX_FILE_SIZE: usize = {};\n",self.max_file_size)
 }
 pub fn validate(&self,project:Option<&Path>)->Result<(),String>{
  let (Some(rel),Some(project))=(&self.uploads,project) else{return Ok(())};
  let mut cur=project.to_path_buf();
  for part in Path::new(rel).components(){cur.push(part);match fs::symlink_metadata(&cur){
   Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink()=>return Err(format!("unsafe upload directory component: {}",cur.display())),
   Ok(_)=>{},Err(e) if e.kind()==io::ErrorKind::NotFound=>{},Err(e)=>return Err(e.to_string()),
  }}
  Ok(())
 }
 pub fn deploy(&self,project:Option<&Path>,output:&Path)->Result<String,String>{
  let Some(rel)=&self.uploads else{return Ok("uploads disabled (no storage.uploads configured)".into())};
  self.validate(project)?;
  let parent=output.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(Path::new("."));
  let dest=parent.join(rel);
  let source=project.map(|root|root.join(rel));
  if let Some(src)=&source {
   // The destination may already be the source folder for in-place builds.
   if src.exists() && dest.exists() && fs::canonicalize(src).ok()==fs::canonicalize(&dest).ok(){return Ok(format!("{} (existing project directory)",dest.display()))}
  }
  let mut curr=parent.to_path_buf();
  for part in Path::new(rel).components(){curr.push(part);match fs::symlink_metadata(&curr){
   Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink()=>return Err(format!("refusing unsafe destination storage path: {}",curr.display())),
   Ok(_)=>{},Err(e) if e.kind()==io::ErrorKind::NotFound=>{fs::create_dir(&curr).map_err(|e|format!("{}: {e}",curr.display()))?;},Err(e)=>return Err(e.to_string()),
  }}
  // Once deployed, the upload directory is durable. Rebuilds must not restore files
  // that an operator has intentionally deleted or alter files already uploaded.
  let marker=dest.join(".phix-storage-initialized");
  if marker.exists(){return Ok(format!("{} (preserved)",dest.display()))}
  if let Some(src)=source.filter(|p|p.is_dir()){copy_initial(&src,&dest,0)?;}
  fs::OpenOptions::new().write(true).create_new(true).open(&marker).or_else(|e|if e.kind()==io::ErrorKind::AlreadyExists{Ok(fs::File::open(&marker)?)}else{Err(e)}).map_err(|e|e.to_string())?;
  Ok(format!("{} (initialized)",dest.display()))
 }
}
fn copy_initial(source:&Path,dest:&Path,depth:usize)->Result<(),String>{
 if depth>24{return Err("storage nesting exceeds 24 directories".into())}
 for entry in fs::read_dir(source).map_err(|e|e.to_string())?{
  let e=entry.map_err(|e|e.to_string())?;
  let t=e.file_type().map_err(|e|e.to_string())?;
  if t.is_symlink(){return Err(format!("refusing symlink in initial storage: {}",e.path().display()))}
  let target=dest.join(e.file_name());
  if t.is_dir(){match fs::symlink_metadata(&target){Ok(m) if !m.is_dir() || m.file_type().is_symlink()=>return Err(format!("unsafe deployment storage: {}",target.display())),Ok(_)=>{},Err(e) if e.kind()==io::ErrorKind::NotFound=>fs::create_dir(&target).map_err(|e|e.to_string())?,Err(e)=>return Err(e.to_string())};copy_initial(&e.path(),&target,depth+1)?;}
  else if t.is_file(){let mut input=fs::File::open(e.path()).map_err(|e|e.to_string())?;let mut output=match fs::OpenOptions::new().write(true).create_new(true).open(&target){Ok(f)=>f,Err(e) if e.kind()==io::ErrorKind::AlreadyExists=>continue,Err(e)=>return Err(e.to_string())};io::copy(&mut input,&mut output).map_err(|e|e.to_string())?;}
 }
 Ok(())
}
