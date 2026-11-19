//! Token-aware, deterministic static module expansion.
//! No source-file code is evaluated by the compiler; includes become compiler inputs.
use std::{collections::HashSet, fs, path::{Path,PathBuf}};
use crate::lexer::{self,Kind,Token};

const MAX_MODULES: usize = 256;
const MAX_EXPANDED_TOKENS: usize = 65534;

pub fn expand(entry: &Path) -> Result<Vec<Token>, String> {
    let mut loader=Loader { seen:HashSet::new(), stack:Vec::new(), modules:0, output:Vec::new() };
    loader.load(entry,false)?;
    let line=loader.output.last().map_or(1,|t|t.line);
    loader.output.push(Token {kind:Kind::End,text:"<eof>".into(),line,file:loader.output.last().and_then(|t|t.file.clone())});
    Ok(loader.output)
}
struct Loader {
    seen:HashSet<PathBuf>,
    stack:Vec<PathBuf>,
    modules:usize,
    output:Vec<Token>,
}
impl Loader {
 fn load(&mut self,path:&Path,once:bool)->Result<(),String>{
    let canonical=fs::canonicalize(path).map_err(|e|format!("{}: {e}",path.display()))?;
    if once && self.seen.contains(&canonical) {return Ok(());}
    if self.stack.contains(&canonical){return Err(format!("{}: circular include detected",canonical.display()));}
    if self.modules>=MAX_MODULES || self.stack.len()>=MAX_MODULES {return Err("module limit exceeded".into());}
    self.modules+=1;
    self.seen.insert(canonical.clone());
    self.stack.push(canonical.clone());
    let result=self.load_contents(&canonical);
    self.stack.pop();
    result
 }
 fn load_contents(&mut self,path:&Path)->Result<(),String>{
    let input=fs::read_to_string(path).map_err(|e|format!("{}: {e}",path.display()))?;
    let tokens=lexer::lex(&input).map_err(|e|format!("{}: {e}",path.display()))?;
    let mut i=0;let mut brace_depth=0i32;
    while i<tokens.len() && tokens[i].kind!=Kind::End {
      let t=&tokens[i];
      let include=if t.kind==Kind::Id {match t.text.as_str(){"include"|"require"=>Some(false),"include_once"|"require_once"=>Some(true),_=>None}}else{None};
      if let Some(once)=include {
        if brace_depth!=0 {return Err(format!("{}:{}: includes inside blocks are not supported yet",path.display(),t.line));}
        let line=t.line;
        i+=1;
        let parenthesized=tokens.get(i).map_or(false,|t|t.text=="(");
        if parenthesized {i+=1;}
        let literal=tokens.get(i).ok_or("missing include path")?;
        if literal.kind!=Kind::Str || literal.text.len()<2 || !literal.text.starts_with('"') || !literal.text.ends_with('"') {
            return Err(format!("{}:{line}: include path must be a double-quoted string literal",path.display()));
        }
        let relative=&literal.text[1..literal.text.len()-1];
        if relative.contains('\\') {return Err(format!("{}:{line}: escape sequences in include paths are not supported",path.display()));}
        i+=1;
        if parenthesized {expect(&tokens,&mut i,")",path,line)?;}
        expect(&tokens,&mut i,";",path,line)?;
        self.load(&path.parent().ok_or("invalid include directory")?.join(relative),once)?;
      }else{
        if t.text=="{"{brace_depth+=1;}
        if t.text=="}"{brace_depth-=1;}
        if brace_depth<0{return Err(format!("{}:{}: unmatched closing brace",path.display(),t.line));}
        if self.output.len()>=MAX_EXPANDED_TOKENS{return Err("expanded token limit exceeded".into());}
        let mut located=t.clone();located.file=Some(path.display().to_string());self.output.push(located);i+=1;
      }
    }
    Ok(())
 }
}
fn expect(tokens:&[Token],i:&mut usize,want:&str,path:&Path,line:usize)->Result<(),String>{
 if tokens.get(*i).map_or(false,|t|t.text==want){*i+=1;Ok(())}else{Err(format!("{}:{line}: expected '{want}' in include",path.display()))}
}

#[cfg(test)]
mod tests {
 use super::*;
 use std::time::{SystemTime,UNIX_EPOCH};
 fn tmp()->PathBuf{let p=std::env::temp_dir().join(format!("phix-modules-test-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir_all(&p).unwrap();p}
 #[test]fn mixed_statements_and_once(){
   let p=tmp();fs::write(p.join("lib.php"),"<?php function add(int $x): int { return $x + 1; }").unwrap();
   fs::write(p.join("main.php"),"<?php require_once(\"lib.php\"); require_once \"lib.php\"; echo add(2);").unwrap();
   let tokens=expand(&p.join("main.php")).unwrap();assert_eq!(tokens.iter().filter(|t|t.text=="function").count(),1);fs::remove_dir_all(p).unwrap();
 }
 #[test]fn include_in_block_rejected(){let p=tmp();fs::write(p.join("main.php"),"<?php if(true) { include \"other.php\"; }").unwrap();assert!(expand(&p.join("main.php")).unwrap_err().contains("inside blocks"));fs::remove_dir_all(p).unwrap();}
 #[test]fn cycle_detected(){let p=tmp();fs::write(p.join("a.php"),"<?php include \"b.php\";").unwrap();fs::write(p.join("b.php"),"<?php include \"a.php\";").unwrap();assert!(expand(&p.join("a.php")).unwrap_err().contains("circular"));fs::remove_dir_all(p).unwrap();}
}
