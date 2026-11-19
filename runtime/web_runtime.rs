// Phix generated-binary runtime, implemented entirely in Rust std.
// Included in generated source by the compiler. Linux-focused HTTP/1.1 prototype.
use std::io::{Read,Write};
use std::net::{Ipv4Addr,SocketAddrV4,TcpListener,TcpStream};
use std::time::Duration;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool,AtomicUsize,Ordering};
use std::sync::Arc;

struct Config{root:Json,port:u16,bind:Ipv4Addr,max_body:usize,max_headers:usize,max_connections:usize,io_timeout:u64}
impl Config{
 fn load()->Result<Self,String>{let executable=std::env::current_exe().map_err(|e|e.to_string())?;let path=executable.parent().ok_or("cannot determine executable directory")?.join("config.json");let root=match std::fs::read_to_string(&path){Ok(t)=>{if t.len()>1024*1024{return Err("config.json exceeds 1 MiB".into());}JsonParser::parse(&t)?},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Json::Object(Vec::new()),Err(e)=>return Err(format!("{}: {e}",path.display()))};
 if !matches!(root,Json::Object(_)){return Err("config.json root must be an object".into());}
 let runtime=match root.get("runtime"){None=>None,Some(Json::Object(_))=>root.get("runtime"),_=>return Err("runtime must be a JSON object".into())};
 let port=match runtime.and_then(|r|r.get("port")){None=>9510,Some(Json::Int(p)) if (1..=65535).contains(p)=>*p as u16,_=>return Err("runtime.port must be an integer 1..65535".into())};
 let bind=match runtime.and_then(|r|r.get("bind")){None=>Ipv4Addr::LOCALHOST,Some(Json::Text(ip))=>ip.parse::<Ipv4Addr>().map_err(|_|"runtime.bind must be an IPv4 address")?,_=>return Err("runtime.bind must be an IPv4 address".into())};let number=|name:&str,default:usize,min:usize,max:usize|->Result<usize,String>{match runtime.and_then(|r|r.get(name)){None=>Ok(default),Some(Json::Int(n)) if *n>=min as i64 && *n<=max as i64=>Ok(*n as usize),_=>Err(format!("runtime.{name} must be an integer between {min} and {max}"))}};
 let max_body=number("max_request_body",16*1024*1024,1,64*1024*1024)?;
 let max_headers=number("max_header_bytes",16384,1024,32768)?;
 let max_connections=number("max_concurrent_requests",32,1,256)?;
 let io_timeout=number("io_timeout_seconds",5,1,30)? as u64;
 Ok(Self{root,port,bind,max_body,max_headers,max_connections,io_timeout})}
 fn value(&self,a:&str,b:&str)->Option<&Json>{let first=self.root.get(a)?;if b.is_empty(){Some(first)}else{first.get(b)}}
}
// Per-request PHP runtime failure, never shared across HTTP worker threads.
thread_local! {
 static PHIX_COMPARISON_ERROR: std::cell::RefCell<Option<Arc<PhixRuntimeError>>> = const { std::cell::RefCell::new(None) };
}
fn phix_checked_equality(left:&Json,right:&Json,strict:bool)->bool {
 match left.phix_checked_array_eq(right,strict) {
  Ok(equal)=>equal,
  Err(message)=>{PHIX_COMPARISON_ERROR.with(|slot|{
   let mut error=slot.borrow_mut();
   if error.is_none(){*error=Some(Arc::new(PhixRuntimeError::new("Error",message)))}
  });false}
 }
}
fn phix_checked_order(left:&Json,right:&Json,op:&str)->bool {
 match left.phix_array_order(right) {
  Ok(Some(order))=>match op {"<"=>order.is_lt(),">"=>order.is_gt(),"<="=>order.is_le(),">="=>order.is_ge(),_=>false},
  Ok(None)=>false,
  Err(message)=>{PHIX_COMPARISON_ERROR.with(|slot|{let mut error=slot.borrow_mut();if error.is_none(){*error=Some(Arc::new(PhixRuntimeError::new("Error",message)))}});false}
 }
}
fn phix_array_operand_type(value:&Json)->&'static str {
 match value {Json::PhpArray(_)|Json::Array(_)=>"array",Json::Int(_)=>"int",Json::Number(_)=>"float",Json::Bool(_)=>"bool",Json::Text(_)=>"string",Json::Null=>"null",Json::Object(_)|Json::Throwable(_)=>"object"}
}
fn phix_array_union_assignment_checked(left:Json,right:Json)->Option<Json> {
 match (left,right) {
  (Json::PhpArray(a),Json::PhpArray(b))=>Some(Json::PhpArray(a.union(&b))),
  (a,b)=>{
   let message=format!("Unsupported operand types: {} + {}",phix_array_operand_type(&a),phix_array_operand_type(&b));
   PHIX_COMPARISON_ERROR.with(|slot|{let mut error=slot.borrow_mut();if error.is_none(){*error=Some(Arc::new(PhixRuntimeError::new("TypeError",message)));}});
   None
  }
 }
}
fn phix_array_union_checked(left:Json,right:Json)->Json {
 phix_array_union_assignment_checked(left,right).unwrap_or(Json::Null)
}
fn phix_throw_builtin(class:&'static str,message:String) {
 PHIX_COMPARISON_ERROR.with(|slot| *slot.borrow_mut()=Some(Arc::new(PhixRuntimeError::new(class,message))));
}
fn phix_throw_builtin_with_code(class:&'static str,message:String,code:i64) {
 PHIX_COMPARISON_ERROR.with(|slot| *slot.borrow_mut()=Some(Arc::new(PhixRuntimeError::with_code(class,message,code))));
}
// Throwable values are identity-bearing runtime objects, never JSON field maps.
fn phix_error_from_json(value:&Json)->Option<Arc<PhixRuntimeError>> {
 match value {Json::Throwable(error)=>Some(Arc::clone(error)),_=>None}
}
fn phix_throw_builtin_with_previous(class:&'static str,message:String,code:i64,previous:Json) {
 let previous=if matches!(previous,Json::Null){None}else{match phix_error_from_json(&previous){Some(v)=>Some(v),None=>{phix_throw_builtin("TypeError","Exception::__construct(): Argument #3 ($previous) must be of type ?Throwable".into());return;}}};
 PHIX_COMPARISON_ERROR.with(|slot| *slot.borrow_mut()=Some(Arc::new(PhixRuntimeError{class,message,code,previous})));
}
fn phix_throw_value(value:Json) {
 if let Some(error)=phix_error_from_json(&value){phix_restore_runtime_error(error);return;}
 phix_throw_builtin("Error","Can only throw objects that implement Throwable".into());
}
fn phix_restore_runtime_error(error:Arc<PhixRuntimeError>){PHIX_COMPARISON_ERROR.with(|slot|*slot.borrow_mut()=Some(error));}
// The built-in Throwable hierarchy is matched by ancestry, not by name prefixes.
// Keep this table aligned with the classes accepted by the source compiler.
fn phix_builtin_parent(class: &str) -> Option<&'static str> {
 match class {
  "Exception" | "Error" => Some("Throwable"),
  "RuntimeException" | "LogicException" => Some("Exception"),
  "InvalidArgumentException" | "DomainException" | "LengthException" | "OutOfRangeException" | "BadFunctionCallException" => Some("LogicException"),
  "BadMethodCallException" => Some("BadFunctionCallException"),
  "OutOfBoundsException" | "OverflowException" | "UnderflowException" | "RangeException" | "UnexpectedValueException" => Some("RuntimeException"),
  "TypeError" | "ValueError" | "ParseError" => Some("Error"),
  _ => None,
 }
}
fn phix_error_matches(error:&PhixRuntimeError, class:&str)->bool {
 let mut current = Some(error.class);
 while let Some(name) = current {
  if name == class { return true; }
  current = phix_builtin_parent(name);
 }
 false
}

fn phix_exception_json(error:&Arc<PhixRuntimeError>)->Json {
 Json::Throwable(Arc::clone(error))
}
fn phix_exception_method_string(value:&Json,key:&str)->String {
 match value {Json::Throwable(error)=>match key {"message"=>error.message.clone(),_=>String::new()},_=>{phix_throw_builtin("Error","Call to a member function on a non-object".into());String::new()}}
}
fn phix_exception_method_int(value:&Json,key:&str)->i64 {
 match value {Json::Throwable(error)=>match key {"code"=>error.code,_=>0},_=>{phix_throw_builtin("Error","Call to a member function on a non-object".into());0}}
}
fn phix_exception_method_previous(value:&Json)->Json {
 match value {Json::Throwable(error)=>error.previous.as_ref().map(|e|Json::Throwable(Arc::clone(e))).unwrap_or(Json::Null),_=>{phix_throw_builtin("Error","Call to a member function on a non-object".into());Json::Null}}
}
fn phix_has_runtime_error()->bool{PHIX_COMPARISON_ERROR.with(|slot|slot.borrow().is_some())}
fn phix_take_comparison_error()->Option<Arc<PhixRuntimeError>>{PHIX_COMPARISON_ERROR.with(|slot|slot.borrow_mut().take())}
// Transition adapter: typed PHP functions now expose explicit outcomes.
// Call-site expression lowering will eventually propagate Result directly.
fn phix_unwrap_function_outcome<T: Default>(outcome: Result<T, Arc<PhixRuntimeError>>) -> T {
 match outcome {
  Ok(value) => value,
  Err(error) => { phix_restore_runtime_error(error); T::default() }
 }
}

trait PhixEcho {fn phix_echo(&self)->String;}
impl PhixEcho for bool{fn phix_echo(&self)->String{if *self{"1".into()}else{String::new()}}}
impl PhixEcho for i64{fn phix_echo(&self)->String{self.to_string()}}
impl PhixEcho for String{fn phix_echo(&self)->String{self.clone()}}
impl PhixEcho for Json{fn phix_echo(&self)->String{self.as_php_string()}}
fn phix_string<T:PhixEcho>(value:&T)->String{value.phix_echo()}
fn phix_escape_html<T:PhixEcho>(value:&T)->String{let text=value.phix_echo();let mut out=String::new();for c in text.chars(){match c{'&'=>out.push_str("&amp;"),'<'=>out.push_str("&lt;"),'>'=>out.push_str("&gt;"),'"'=>out.push_str("&quot;"),'\''=>out.push_str("&#039;"),_=>out.push(c)}}out}
fn decode_query_component(raw:&str)->Result<String,()>{
 let b=raw.as_bytes();let mut result=Vec::with_capacity(b.len());let mut i=0;
 while i<b.len(){match b[i]{b'+'=>{result.push(b' ');i+=1;},b'%'=>{if i+2>=b.len(){return Err(());}let h=|v:u8|->Option<u8>{match v{b'0'..=b'9'=>Some(v-b'0'),b'a'..=b'f'=>Some(v-b'a'+10),b'A'..=b'F'=>Some(v-b'A'+10),_=>None}};let hi=h(b[i+1]).ok_or(())?;let lo=h(b[i+2]).ok_or(())?;result.push(hi*16+lo);i+=3;},v=>{result.push(v);i+=1;}}}
 String::from_utf8(result).map_err(|_|())
}
fn query_values(query:&str)->Result<Json,()>{
 let mut values:Vec<(String,Json)>=Vec::new();
 for (index,part) in query.split('&').filter(|p|!p.is_empty()).enumerate(){if index>=1024{return Err(());}let (key,value)=part.split_once('=').unwrap_or((part,""));let key=decode_query_component(key)?;let value=decode_query_component(value)?;
 // The first iteration supports scalar PHP-style query strings. Bracket-array parsing is a later milestone.
 if let Some((_,v))=values.iter_mut().find(|(k,_)|k==&key){*v=Json::Text(value);}else{values.push((key,Json::Text(value)));}}
 Ok(Json::Object(values))
}
struct UploadedFile {token:String, data:Vec<u8>, moved:bool}
struct RequestContext<'a>{config:&'a Config,output:Vec<u8>,content_type:&'static str,get:Json,server:Json, files:Json, uploads:Vec<UploadedFile>}
impl<'a> RequestContext<'a>{
 fn new(config:&'a Config,get:Json,method:&str,uri:&str,query:&str,script:&str, files:Json, uploads:Vec<UploadedFile>)->Self{
 let server=Json::Object(vec![("REQUEST_METHOD".into(),Json::Text(method.into())),("REQUEST_URI".into(),Json::Text(uri.into())),("QUERY_STRING".into(),Json::Text(query.into())),("SCRIPT_NAME".into(),Json::Text(script.into())),("SERVER_PORT".into(),Json::Text(config.port.to_string()))]);
 Self{config,output:Vec::new(),content_type:"text/html; charset=utf-8",get,server,files,uploads}
 }
 fn echo<T:PhixEcho>(&mut self,value:&T){self.output.extend_from_slice(value.phix_echo().as_bytes());}
 fn cfg_value(&self,keys:&[&str])->Json{
   let mut value=&self.config.root;
   for key in keys {match value.get(key){Some(found)=>value=found,None=>return Json::Null}}
   value.clone()
 }
 fn get_value(&self,keys:&[&str])->Json{Self::nested(&self.get,keys)}
 fn server_value(&self,keys:&[&str])->Json{Self::nested(&self.server,keys)}
 fn files_value(&self,keys:&[&str])->Json{Self::nested(&self.files,keys)}
 fn move_uploaded_file(&mut self,token:&Json,destination:&str)->bool{
  // Storage is opt-in: never write uploads in an unconfigured application.
  let Some(upload_dir)=PHIX_UPLOAD_DIR else{return false};
  let Json::Text(token)=token else{return false};
  let Some(file)=self.uploads.iter_mut().find(|u|&u.token==token&&!u.moved) else{return false};
  // Accept both paths relative to the configured root and paths prefixed with it.
  let destination=destination.strip_prefix(upload_dir).and_then(|tail|tail.strip_prefix('/')).unwrap_or(destination);
  let relative=std::path::Path::new(destination);
  if destination.is_empty() || destination.contains('\\') ||
     !relative.components().all(|c|matches!(c,std::path::Component::Normal(_))) ||
     destination.split('/').any(|s|s.starts_with('.') || s.is_empty()) {return false}
  let Ok(exe)=std::env::current_exe() else{return false};
  let Some(parent)=exe.parent() else{return false};
  let mut path=parent.to_path_buf();
  for c in std::path::Path::new(upload_dir).components() {
   let std::path::Component::Normal(name)=c else{return false};
   path.push(name);
   match std::fs::symlink_metadata(&path){
    Ok(m) if m.is_dir() && !m.file_type().is_symlink()=>{},
    _=>return false,
   }
  }
  let components:Vec<_>=relative.components().collect();
  for (i,c) in components.iter().enumerate(){
   let std::path::Component::Normal(name)=c else{return false};
   path.push(name);
   if i+1<components.len(){
    match std::fs::symlink_metadata(&path){
     Ok(m) if m.is_dir() && !m.file_type().is_symlink()=>{},
     _=>return false, // no implicit directory creation or symlink traversal
    }
   }
  }
  // create_new prevents overwrites and prevents following an existing destination symlink.
  match std::fs::OpenOptions::new().write(true).create_new(true).open(&path){
   Ok(mut out)=>{
    if out.write_all(&file.data).is_err(){drop(out);let _=std::fs::remove_file(&path);return false}
    file.moved=true;true
   },Err(_)=>false
  }
 }
 fn nested(root:&Json,keys:&[&str])->Json{let mut value=root;for key in keys{match value.get(key){Some(v)=>value=v,None=>return Json::Null}}value.clone()}
 fn asset_text(&self,path:&str)->String{
   match self.find_asset(path) {
     Some(data) => match std::str::from_utf8(data) {
       Ok(text) => text.to_owned(),
       Err(_) => {eprintln!("phix: asset_text: invalid UTF-8 in {path:?}; use asset_write() for binary resources");String::new()}
     },
     None => String::new()
   }
 }
 fn asset_bytes(&self,path:&str)->Vec<u8>{self.find_asset(path).map(|data|data.to_vec()).unwrap_or_default()}
 fn asset_write(&mut self,path:&str){if let Some(data)=self.find_asset(path){if self.output.is_empty(){self.content_type=mime(path);}self.output.extend_from_slice(data);}}
 fn asset_exists(&self,path:&str)->bool{self.find_asset(path).is_some()}
 fn asset_size(&self,path:&str)->i64{self.find_asset(path).map(|b|b.len() as i64).unwrap_or(-1)}
 fn find_asset(&self,path:&str)->Option<&'static [u8]>{PHIX_ASSETS.iter().find(|(p,_)|*p==path).map(|(_,d)|*d)}
}
fn mime(path:&str)->&'static str{match PathBuf::from(path).extension().and_then(|x|x.to_str()).unwrap_or(""){"css"=>"text/css","js"=>"text/javascript","html"=>"text/html","svg"=>"image/svg+xml","png"=>"image/png","jpg"|"jpeg"=>"image/jpeg","json"=>"application/json","txt"=>"text/plain",_=>"application/octet-stream"}}
fn send(stream:&mut TcpStream,code:u16,kind:&str,data:&[u8],head:bool)->std::io::Result<()>{send_with_location(stream,code,kind,data,head,None)}
fn send_with_location(stream:&mut TcpStream,code:u16,kind:&str,data:&[u8],head:bool,location:Option<&str>)->std::io::Result<()>{
 let reason=match code{200=>"OK",301=>"Moved Permanently",302=>"Found",303=>"See Other",307=>"Temporary Redirect",308=>"Permanent Redirect",400=>"Bad Request",413=>"Payload Too Large",431=>"Request Header Fields Too Large",501=>"Not Implemented",403=>"Forbidden",404=>"Not Found",405=>"Method Not Allowed",_=>"Error"};
 let mut hdr=format!("HTTP/1.1 {code} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n",data.len());
 if let Some(url)=location{hdr.push_str(&format!("Location: {url}\r\n"));}
 hdr.push_str("\r\n");stream.write_all(hdr.as_bytes())?;if !head{stream.write_all(data)?;}Ok(())
}
fn path_safe(raw:&str)->Result<String,()>{
 if !raw.starts_with('/')||raw.starts_with("//")||raw.len()>4096 {return Err(());}
 let data=raw.as_bytes();let mut out=Vec::new();let mut i=0;
 while i<data.len(){let v=if data[i]==b'%' {
   if i+2>=data.len(){return Err(())}
   let hex=|c:u8|->Option<u8>{match c{b'0'..=b'9'=>Some(c-b'0'),b'A'..=b'F'=>Some(c-b'A'+10),b'a'..=b'f'=>Some(c-b'a'+10),_=>None}};
   let byte=hex(data[i+1]).ok_or(())?*16+hex(data[i+2]).ok_or(())?;
   // Encoded separators and path-control characters must not bypass access rules.
   if byte==b'/'||byte==b'\\'||byte==b'.'||byte==b'%' {return Err(())}
   i+=3;byte
 }else{let v=data[i];i+=1;v};
 if v==b'\\'||v<32||v==127||v==b'?'||v==b'#'{return Err(())}out.push(v);
 }
 let path=String::from_utf8(out).map_err(|_|())?;
 if path.split('/').any(|part|part=="."||part==".."||part.starts_with('.')&&!part.is_empty()) {return Err(())}
 Ok(path)
}
fn route_matches(pattern:&str,path:&str)->Option<Vec<(String,String)>>{
 let a:Vec<_>=pattern.split('/').collect();let b:Vec<_>=path.split('/').collect();
 if a.len()!=b.len(){return None}
 let mut values=Vec::new();for (x,y) in a.iter().zip(b.iter()) {
  if let Some(name)=x.strip_prefix(':'){if name.is_empty()||y.is_empty(){return None}values.push((name.to_string(),y.to_string()));}
  else if x!=y{return None}
 }Some(values)
}
fn forbidden(path:&str)->bool{
 if path.split('/').any(|s|s.starts_with('.')&&!s.is_empty()){return true}
 let file=path.rsplit('/').next().unwrap_or("");
 if ["config.json","phix.json","routes.json","composer.json","composer.lock"].contains(&file) || [".env",".key",".pem",".p12"].iter().any(|s|file.ends_with(s)){return true}
 for pat in PHIX_DENY{
  if let Some(prefix)=pat.strip_suffix("/**") {if path==prefix||path.starts_with(&format!("{prefix}/")){return true}}
  else if route_matches(pat,path).is_some(){return true}
 }
 false
}
fn url_escape_segment(s:&str)->String{
 let mut out=String::new();for byte in s.as_bytes(){
   if byte.is_ascii_alphanumeric()||matches!(*byte,b'-'|b'_'|b'.'|b'~') {out.push(*byte as char)}
   else {out.push_str(&format!("%{byte:02X}"));}
 }out
}
fn expand_target(template:&str,captures:&[(String,String)])->String{
 let mut target=template.to_string();for (key,value) in captures{
  target=target.replace(&format!(":{key}"),&url_escape_segment(value));
 }target
}

fn multipart_uploads(content_type:&str,body:&[u8])->Result<(Json,Vec<UploadedFile>),()> {
 let boundary=content_type.split(';').map(str::trim).find_map(|s|s.strip_prefix("boundary=")).ok_or(())?.trim_matches('"');
 if boundary.is_empty() || boundary.len()>70 || !boundary.bytes().all(|x|x.is_ascii_alphanumeric() || b"'()+_,-./:=?".contains(&x)){return Err(())}
 let marker=format!("--{boundary}").into_bytes();
 let separator=format!("\r\n--{boundary}").into_bytes();
 if !body.starts_with(&marker){return Err(())}
 let mut cursor=marker.len();
 let mut values:Vec<(String,Json)>=Vec::new();let mut uploads=Vec::new();
 loop {
  if body.get(cursor..cursor+2)==Some(&b"--"[..]){break;}
  if body.get(cursor..cursor+2)!=Some(&b"\r\n"[..]){return Err(())}cursor+=2;
  let header_end=body[cursor..].windows(4).position(|w|w==b"\r\n\r\n").ok_or(())?+cursor;
  if header_end-cursor>8192{return Err(())}
  let head=std::str::from_utf8(&body[cursor..header_end]).map_err(|_|())?;
  cursor=header_end+4;
  let next=body[cursor..].windows(separator.len()).position(|w|w==separator.as_slice()).ok_or(())?+cursor;
  let data=&body[cursor..next];cursor=next+2+marker.len();
  let disposition=head.lines().find(|l|l.to_ascii_lowercase().starts_with("content-disposition:")).ok_or(())?;
  if !disposition.to_ascii_lowercase().contains("form-data"){return Err(())}
  let param=|key:&str|->Option<String>{
   disposition.split(';').skip(1).map(str::trim).find_map(|p|p.strip_prefix(key).map(|v|v.trim_matches('"').to_string()))
  };
  let field=param("name=").ok_or(())?;
  if field.is_empty() || field.len()>128 {return Err(())}
  let Some(original_name)=param("filename=") else{continue;}; // normal form fields are not uploaded files
  if original_name.is_empty(){continue;}
  if uploads.len()>=16 || data.len()>PHIX_UPLOAD_MAX_FILE_SIZE{return Err(())}
  // Browsers may send a path. Expose only the basename in $_FILES.
  let filename=original_name.rsplit(['/', '\\']).next().unwrap_or("").to_string();
  if filename.is_empty(){return Err(())}
  let content_type=head.lines().find_map(|s|s.to_ascii_lowercase().starts_with("content-type:").then_some(s.split_once(':').map(|v|v.1.trim()).unwrap_or("application/octet-stream"))).unwrap_or("application/octet-stream").to_string();
  let token=format!("phix-upload-{}",uploads.len());
  values.push((field,Json::Object(vec![
   ("name".into(),Json::Text(filename)),("type".into(),Json::Text(content_type)),
   ("size".into(),Json::Int(data.len() as i64)),("tmp_name".into(),Json::Text(token.clone())),
   ("error".into(),Json::Int(0))
  ])));
  uploads.push(UploadedFile{token,data:data.to_vec(),moved:false});
 }
 Ok((Json::Object(values),uploads))
}

fn read_http_request(stream:&mut TcpStream,config:&Config)->std::io::Result<Result<(String,String,Vec<(String,String)>,Vec<u8>),u16>> {
 // Read until the complete header arrives; never interpret an incomplete header as a request.
 let mut received=Vec::new();let mut buf=[0u8;8192];
 let header_end=loop{
  if let Some(i)=received.windows(4).position(|v|v==b"\r\n\r\n"){break i+4}
  if received.len()>=config.max_headers{return Ok(Err(431))}
  let n=stream.read(&mut buf[..(config.max_headers-received.len()).min(8192)])?;if n==0{return Ok(Err(400))}received.extend_from_slice(&buf[..n]);
 };
 if header_end>config.max_headers{return Ok(Err(431))}
 let Ok(header)=std::str::from_utf8(&received[..header_end]) else{return Ok(Err(400))};
 let mut lines=header.split("\r\n");let request=lines.next().unwrap_or("");let p:Vec<&str>=request.split_whitespace().collect();
 if p.len()!=3 || !(p[2]=="HTTP/1.0" || p[2]=="HTTP/1.1") || !p[1].starts_with('/') || p[1].bytes().any(|b|b<=32 || b==127){return Ok(Err(400))}
 let mut headers=Vec::new();let mut length=None;let mut transfer_encoding=false;let mut host_count=0usize;
 for line in lines.filter(|l|!l.is_empty()){
  let Some((key,value))=line.split_once(':') else{return Ok(Err(400))};
  let key=key.to_ascii_lowercase();let value=value.trim().to_string();
  if headers.len()>=100 || key.is_empty() || !key.bytes().all(|b|b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)) || value.bytes().any(|b|b==0 || b==b'\r' || b==b'\n' || (b<32 && b!=b'\t')) {return Ok(Err(400))}
  if key=="host"{host_count+=1;if host_count>1 || value.is_empty(){return Ok(Err(400))}}
  if key=="content-length"{
   if length.is_some(){return Ok(Err(400))}
   length=match value.parse::<usize>(){Ok(n)=>Some(n),Err(_)=>return Ok(Err(400))};
  }
  if key=="transfer-encoding"{transfer_encoding=true;}
  headers.push((key,value));
 }
 if p[2]=="HTTP/1.1" && host_count!=1{return Ok(Err(400))}
 if transfer_encoding{return Ok(Err(400))} // Reject all TE framing, especially TE+CL ambiguity.
 if PHIX_UPLOAD_DIR.is_none() && headers.iter().any(|(k,v)|k=="content-type"&&v.to_ascii_lowercase().starts_with("multipart/form-data")){return Ok(Err(403))}
 let length=length.unwrap_or(0);
 if length>config.max_body || length>PHIX_UPLOAD_MAX_FILE_SIZE.saturating_mul(16).saturating_add(1024*1024).min(64*1024*1024){return Ok(Err(413))}
 let mut body=received[header_end..].to_vec();
 if body.len()>length{return Ok(Err(400))}
 while body.len()<length{let need=(length-body.len()).min(buf.len());let n=stream.read(&mut buf[..need])?;if n==0{return Ok(Err(400))}body.extend_from_slice(&buf[..n]);}
 Ok(Ok((p[0].to_string(),p[1].to_string(),headers,body)))
}

fn handle(stream:&mut TcpStream,config:&Config)->std::io::Result<()>{
 stream.set_read_timeout(Some(Duration::from_secs(config.io_timeout)))?;stream.set_write_timeout(Some(Duration::from_secs(config.io_timeout)))?;
 let (method,request_uri,headers,body)=match read_http_request(stream,config)? {
  Ok(v)=>v,Err(status)=>return send(stream,status,"text/plain",b"Invalid HTTP request",false)
 };
 if method!="GET"&&method!="HEAD"&&method!="POST"{return send(stream,405,"text/plain",b"Method Not Allowed",false)}
 let head=method=="HEAD";let raw=request_uri.as_str();let (raw_path,original_query)=raw.split_once('?').unwrap_or((raw,""));
 let path=match path_safe(raw_path){Ok(p)=>p,Err(_)=>return send(stream,400,"text/plain",b"Bad Request",head)};
 if forbidden(&path){return send(stream,403,"text/plain",b"Forbidden",head)}
 for (pattern,target,status) in PHIX_REDIRECTS{
  if let Some(captures)=route_matches(pattern,&path){
   let url=expand_target(target,&captures);
   return send_with_location(stream,*status,"text/plain",b"Redirect",head,Some(&url));
  }
 }
 let mut selected=if path=="/"{["/index.php","/index.phx","/main.php","/main.phx"].iter().find(|x|PHIX_PUBLIC_ROUTES.contains(x)).unwrap_or(&"/").to_string()}else{path.clone()};
 let mut rewritten_query=String::new();
 for (pattern,target,_) in PHIX_REWRITES{
  if let Some(captures)=route_matches(pattern,&path){
   let expanded=expand_target(target,&captures);
   let (newpath,newquery)=expanded.split_once('?').unwrap_or((&expanded,""));
   selected=newpath.to_string();rewritten_query=newquery.to_string();break;
  }
 }
 if forbidden(&selected){return send(stream,403,"text/plain",b"Forbidden",head)}
 // Static files are only exposed beneath the configured document root.
 // Internal assets may remain embedded for asset_text()/asset_write(), but are not public.
 if selected==path {
  let key=format!("{}{path}",if PHIX_DOCUMENT_ROOT.is_empty(){""}else{PHIX_DOCUMENT_ROOT}).trim_start_matches('/').to_string();
  if !key.ends_with('/') {
   if let Some((_,asset))=PHIX_ASSETS.iter().find(|(p,_)|*p==key){if method=="POST"{return send(stream,405,"text/plain",b"Method Not Allowed",false)}return send(stream,200,mime(&key),asset,head)}
  }
 }
 if !PHIX_PUBLIC_ROUTES.contains(&selected.as_str()){
  if let Some(fallback)=PHIX_FALLBACK {selected=fallback.into();}else{return send(stream,404,"text/plain",b"Not Found",head)}
 }
 if forbidden(&selected){return send(stream,403,"text/plain",b"Forbidden",head)}
 let query=if rewritten_query.is_empty(){original_query.to_string()}else if original_query.is_empty(){rewritten_query}else{format!("{rewritten_query}&{original_query}")};
 let get=match query_values(&query){Ok(v)=>v,Err(_)=>return send(stream,400,"text/plain",b"Bad Request",head)};
 let content_type=headers.iter().find(|(k,_)|k=="content-type").map(|(_,v)|v.as_str()).unwrap_or("");
 let (files,uploads)=if method=="POST" && content_type.to_ascii_lowercase().starts_with("multipart/form-data"){
  if PHIX_UPLOAD_DIR.is_none(){return send(stream,403,"text/plain",b"Uploads disabled",head)}
  match multipart_uploads(content_type,&body){Ok(parsed)=>parsed,Err(_)=>return send(stream,400,"text/plain",b"Invalid multipart upload",head)}
 }else{(Json::Object(Vec::new()),Vec::new())};
 let mut ctx=RequestContext::new(config,get,&method,raw,&query,&selected,files,uploads);
 // Clear stale state even if a worker is reused in a future runtime.
 let _=phix_take_comparison_error();
 if let Some(index)=PHIX_PUBLIC_ROUTES.iter().position(|p|*p==selected){phix_route_run(index,&mut ctx)}else{return send(stream,404,"text/plain",b"Not Found",head)}
 if let Some(message)=phix_take_comparison_error(){
  eprintln!("phix: PHP {}: {}",message.class,message.message);
  // Never return partially rendered output after a fatal comparison error.
  return send(stream,500,"text/plain; charset=utf-8",format!("Fatal error: Uncaught {}: {}\n",message.class,message.message).as_bytes(),head);
 }
 send(stream,200,ctx.content_type,&ctx.output,head)
}

static PHIX_RUNNING:AtomicBool=AtomicBool::new(true);
#[cfg(unix)]
mod unix_signals{use super::*;extern "C"{fn signal(sig:i32,handler:usize)->usize;}extern "C" fn stop(_:i32){PHIX_RUNNING.store(false,Ordering::SeqCst);}pub fn install(){unsafe{signal(2,stop as usize);signal(15,stop as usize);}}}
fn phix_serve()->Result<(),String>{
 let config=Arc::new(Config::load()?);
 let listener=TcpListener::bind(SocketAddrV4::new(config.bind,config.port)).map_err(|e|format!("listen: {e}"))?;
 listener.set_nonblocking(true).map_err(|e|e.to_string())?;
 #[cfg(unix)]unix_signals::install();
 eprintln!("Phix HTTP listening on http://{}:{}",config.bind,config.port);
 let active=Arc::new(AtomicUsize::new(0));
 while PHIX_RUNNING.load(Ordering::SeqCst){
  match listener.accept(){
   Ok((mut stream,_))=>{
    // Reserve a slot before spawning a worker; reject overload without allocating a thread.
    let reserved=active.fetch_update(Ordering::AcqRel,Ordering::Acquire,|n|if n<config.max_connections{Some(n+1)}else{None}).is_ok();
    if !reserved {let _=stream.set_write_timeout(Some(Duration::from_secs(1)));let _=send(&mut stream,503,"text/plain",b"Server Busy",false);continue;}
    let cfg=Arc::clone(&config);let counter=Arc::clone(&active);
    let spawn=std::thread::Builder::new().name("phix-request".into()).spawn(move||{
     struct Slot(Arc<AtomicUsize>);impl Drop for Slot{fn drop(&mut self){self.0.fetch_sub(1,Ordering::AcqRel);}}
     let _slot=Slot(counter);
     if let Err(e)=handle(&mut stream,&cfg){eprintln!("phix: request: {e}");}
    });
    if let Err(e)=spawn {active.fetch_sub(1,Ordering::AcqRel);eprintln!("phix: cannot spawn request worker: {e}");}
   },
   Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(30)),
   Err(e) if e.kind()==std::io::ErrorKind::Interrupted=>{},
   Err(e)=>return Err(format!("accept: {e}"))
  }
 }
 // Existing requests may complete while the process shuts down.
 Ok(())
}
