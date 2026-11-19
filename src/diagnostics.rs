//! Human-readable source diagnostics for PHP/PHX build errors.
use std::{fs,path::Path};
use crate::lexer::Token;

pub fn render(message:&str,tokens:&[Token],entry:&Path)->String {
    let line=message.strip_prefix("line ").and_then(|rest|rest.split(':').next()).and_then(|s|s.parse::<usize>().ok());
    let Some(line)=line else {return format!("error: {message}");};
    let token=tokens.iter().find(|t|t.line==line && t.file.is_some());
    let file=token.and_then(|t|t.file.as_deref()).map(Path::new).unwrap_or(entry);
    let detail=message.split_once(':').map_or(message,|(_,d)|d.trim());
    let mut output=format!("error: {detail}\n --> {}:{line}\n",file.display());
    if let Ok(src)=fs::read_to_string(file) {
        if let Some(content)=src.lines().nth(line.saturating_sub(1)) {
            output.push_str(&format!("  |\n{line:>3} | {content}\n  |\n"));
        }
    }
    if detail.contains("expected ';'") {
        output.push_str("help: add a semicolon (;) after the statement\n");
    }
    output
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn semicolon_hint(){let rendered=render("line 2: expected ';', got '<eof>'",&[],Path::new("example.php"));assert!(rendered.contains("help: add a semicolon"));assert!(rendered.contains("example.php:2"));}
 #[test]fn general_error(){assert_eq!(render("unknown function foo",&[],Path::new("a.php")),"error: unknown function foo");}
}
