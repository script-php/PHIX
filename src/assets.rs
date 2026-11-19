use std::{fs,path::{Path,PathBuf}};
fn scan(root:&Path,rel:&Path,out:&mut Vec<(String,Vec<u8>)>,total:&mut usize,uploads:Option<&Path>)->Result<(),String>{
 for ent in fs::read_dir(root.join(rel)).map_err(|e|e.to_string())?{let ent=ent.map_err(|e|e.to_string())?;let ty=ent.file_type().map_err(|e|e.to_string())?;if ty.is_symlink(){continue;}let name=ent.file_name().to_string_lossy().into_owned();if name.starts_with('.') {continue;}let p=rel.join(&name);
 if uploads.is_some_and(|dir|p==dir || p.starts_with(dir)){continue;}
 if ty.is_dir(){if ["build","dist","vendor","node_modules","target"].contains(&name.as_str()){continue;}scan(root,&p,out,total,uploads)?;continue;}
 if !ty.is_file(){continue;}if [".php",".phx",".o",".exe",".so",".a",".env",".key",".pem"].iter().any(|x|name.ends_with(x))||["config.json","phix.json","Makefile","phix"].contains(&name.as_str()){continue;}
 let data=fs::read(root.join(&p)).map_err(|e|e.to_string())?;*total=total.checked_add(data.len()).ok_or("embedded asset size overflow")?;if data.len()>16*1024*1024||*total>64*1024*1024||out.len()>=4096{return Err("embedded asset limits exceeded".into());}out.push((p.to_string_lossy().replace('\\',"/"),data));
 }Ok(())
}
pub fn emit(project:Option<&Path>,uploads:Option<&str>)->Result<String,String>{let mut entries=Vec::new();let mut total=0;if let Some(root)=project{scan(root,&PathBuf::new(),&mut entries,&mut total,uploads.map(Path::new))?;}entries.sort_by(|a,b|a.0.cmp(&b.0));let mut s=String::from("\nstatic PHIX_ASSETS: &[(&str, &[u8])] = &[\n");for(name,bytes)in entries {s.push_str(&format!("({:?}, &[",name));for b in bytes{s.push_str(&format!("{b},"));}s.push_str("]),\n");}s.push_str("];\n");Ok(s)}
