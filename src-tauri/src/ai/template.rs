//! 简单模板引擎：把 `{{key}}` 替换为 vars[key]。

use crate::error::{AppError, AppResult};
use std::collections::HashMap;

pub fn render(template: &str, vars: &HashMap<String, String>) -> AppResult<String> {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // 找 }}
            if let Some(end) = find_close(&template[i + 2..]) {
                let key = template[i + 2..i + 2 + end].trim();
                if let Some(val) = vars.get(key) {
                    out.push_str(val);
                } else {
                    return Err(AppError::Config(format!("模板变量缺失: {key}")));
                }
                i += 2 + end + 2;
                continue;
            }
        }
        // 普通字符（注意 utf-8）
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    Ok(out)
}

fn find_close(s: &str) -> Option<usize> {
    s.find("}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_replace() {
        let mut vars = HashMap::new();
        vars.insert("name".into(), "world".into());
        let out = render("hello {{name}}!", &vars).unwrap();
        assert_eq!(out, "hello world!");
    }
    #[test]
    fn missing_var_err() {
        let vars = HashMap::new();
        assert!(render("hi {{x}}", &vars).is_err());
    }
}
