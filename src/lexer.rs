#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Kind{Id,Var,Num,Str,Sym,End}
#[derive(Clone,Debug)]
pub struct Token{pub kind:Kind,pub text:String,pub line:usize,pub file:Option<String>}
pub fn lex(source:&str)->Result<Vec<Token>,String>{
 let b=source.as_bytes();let(mut i,mut line)=(0,1);let mut out=Vec::new();
 while i<b.len(){let start=i;let c=b[i];
 if c.is_ascii_whitespace(){if c==b'\n'{line+=1;}i+=1;continue;}
 if b[i..].starts_with(b"<?php"){i+=5;continue;}
 if b[i..].starts_with(b"//")||c==b'#'{while i<b.len()&&b[i]!=b'\n'{i+=1;}continue;}
 if b[i..].starts_with(b"/*"){i+=2;while i+1<b.len()&&!b[i..].starts_with(b"*/"){if b[i]==b'\n'{line+=1;}i+=1;}if i+1>=b.len(){return Err(format!("line {line}: unterminated comment"));}i+=2;continue;}
 let kind=if c==b'$'{i+=1;let s=i;while i<b.len()&&(b[i].is_ascii_alphanumeric()||b[i]==b'_'){i+=1;}if s==i || !(b[s].is_ascii_alphabetic() || b[s]==b'_'){return Err(format!("line {line}: invalid variable name"));}out.push(Token{kind:Kind::Var,text:String::from_utf8_lossy(&b[s..i]).into_owned(),line,file:None});continue;}
 else if c.is_ascii_alphabetic()||c==b'_'{i+=1;while i<b.len()&&(b[i].is_ascii_alphanumeric()||b[i]==b'_'){i+=1;}Kind::Id}
 else if c.is_ascii_digit(){i+=1;while i<b.len()&&b[i].is_ascii_digit(){i+=1;}Kind::Num}
 else if c==b'\''{
  i+=1;let mut value=String::new();let mut terminated=false;
  while i<b.len(){let next=b[i];if next==b'\''{i+=1;terminated=true;break;}
    if next==b'\\' && i+1<b.len() && (b[i+1]==b'\\'||b[i+1]==b'\''){value.push(b[i+1] as char);i+=2;continue;}
    if next==b'\n'{line+=1;}
    if next<128 {value.push(next as char);i+=1;} else {let decoded=std::str::from_utf8(&b[i..]).map_err(|_|format!("line {line}: invalid UTF-8"))?;let ch=decoded.chars().next().ok_or("invalid string")?;value.push(ch);i+=ch.len_utf8();}
  }
  if !terminated{return Err(format!("line {line}: unterminated single-quoted string"));}
  out.push(Token{kind:Kind::Str,text:format!("{value:?}"),line,file:None});continue;
 }
 else if c==b'"'{i+=1;while i<b.len()&&b[i]!=b'"'{if b[i]==b'\\'{i+=1;}if i<b.len()&&b[i]==b'\n'{line+=1;}i+=1;}if i>=b.len(){return Err(format!("line {line}: unterminated string"));}i+=1;Kind::Str}
 else {let mut len=1;if b[i..].starts_with(b"===")||b[i..].starts_with(b"!==")||b[i..].starts_with(b"??="){len=3;}else if [b"==".as_slice(),b"!=",b"<=",b">=",b"&&",b"||",b"=>",b"+=",b"->",b"??"].iter().any(|s|b[i..].starts_with(s)){len=2;}else if !b"(){}[];,:?=+-*/.<>!&|".contains(&c){return Err(format!("line {line}: unexpected character '{}'",c as char));}i+=len;Kind::Sym};
 out.push(Token{kind,text:String::from_utf8_lossy(&b[start..i]).into_owned(),line,file:None});
 }
 out.push(Token{kind:Kind::End,text:"<eof>".into(),line,file:None});Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn php_variable_names() {
        assert!(lex("<?php $name = 1; $_valid2 = 2;").is_ok());
        for input in ["<?php $123 = 1;", "<?php $1abc = 1;", "<?php $ = 1;"] {
            assert!(lex(input).is_err(), "should reject {input}");
        }
    }
}

#[cfg(test)]
mod formatting_tests {
    use super::*;
    fn significant(src: &str) -> Vec<(Kind, String)> {
        lex(src).unwrap().into_iter().map(|t| (t.kind, t.text)).collect()
    }

    #[test]
    fn formatting_is_not_syntax() {
        let pretty = "<?php\n$value = 2 + 3;\nif ($value > 0) { echo 'done'; }";
        let compact = "<?php $value=2+3;if($value>0){echo 'done';}";
        let unusual = "<?php\n$value\n=\n2\n+\n3\n;\nif\n(\n$value\n>\n0\n)\n{\necho\n'done'\n;\n}";
        assert_eq!(significant(pretty), significant(compact));
        assert_eq!(significant(pretty), significant(unusual));
    }

    #[test]
    fn php_comments_and_line_endings_are_ignored_between_tokens() {
        let base = "<?php $a = 1; echo $a;";
        let commented = "<?php\r\n/* block\r\n comment */\t$a /* separator */ = 1; # hash comment\r\n// other comment\r\necho\t$a;";
        assert_eq!(significant(base), significant(commented));
    }

    #[test]
    fn whitespace_inside_strings_is_preserved() {
        let a = significant("<?php echo 'hello   world';");
        let b = significant("<?php echo 'hello world';");
        assert_ne!(a,b);
        let multiline = significant("<?php echo 'hello\nworld';");
        assert_ne!(a,multiline);
    }

    #[test]
    fn operators_and_identifiers_cannot_be_split() {
        assert_ne!(significant("<?php echo 1===1;"), significant("<?php echo 1= = =1;"));
        assert_ne!(significant("<?php echo $myVar;"), significant("<?php echo $my Var;"));
    }
}

#[cfg(test)]
mod exception_method_lexer_tests {
 use super::*;
 #[test]
 fn exception_method_arrow_is_one_token() {
  let tokens=lex("<?php echo $e->getMessage();").unwrap();
  assert!(tokens.iter().any(|t|t.text=="->"));
 }
}
