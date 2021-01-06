use std::str::Chars;

use crate::error::Error;
use crate::scope::SchemaScope;
use serde::Serialize;
use serde_json::Value;

pub fn each_node_mut<F>(
    root: &mut Value,
    context: &mut SchemaScope,
    path: &str,
    mut f: F,
) -> Result<(), Error>
where
    F: FnMut(&mut Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    let parts = path
        .trim_matches('/')
        .split('/')
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    each_mut(root, context, &parts, 0, &mut vec![], &mut f)
}

fn each_mut<F>(
    node: &mut Value,
    context: &mut SchemaScope,
    path: &[String],
    index: usize,
    parts: &mut Vec<String>,
    f: &mut F,
) -> Result<(), Error>
where
    F: FnMut(&mut Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    match path.get(index) {
        None => f(node, parts, context),
        Some(search) => {
            if let [type_, search_key] = &search.split(':').into_iter().collect::<Vec<&str>>()[..] {
                match *search_key {
                    "*" => match node {
                        Value::Object(ref mut map) => {
                            for (key, value) in map {
                                context.push_str(*type_, key);

                                parts.push(key.clone());
                                each_mut(value, context, path, index + 1, parts, f)?;
                                parts.pop();

                                context.pop();
                            }

                            Ok(())
                        }
                        _ => Err(Error::NotImplemented),
                    },
                    real_path => {
                        context.push_str(*type_, real_path);

                        if let Some(ref mut found) = node.pointer_mut(&["/", real_path].join("")) {
                            each_mut(found, context, path, index + 1, parts, f)?;
                        }

                        context.pop();

                        Ok(())
                    }
                }
            } else {
                panic!("Incorrect path: {}", search);
            }
        }
    }
}

pub fn each_node<F>(
    root: &Value,
    context: &mut SchemaScope,
    path: &str,
    mut f: F,
) -> Result<(), Error>
where
    F: FnMut(&Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    let parts = path
        .trim_matches('/')
        .split('/')
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    each(root, context, &parts, 0, &mut vec![], &mut f)
}

fn each<F>(
    node: &Value,
    context: &mut SchemaScope,
    path: &[String],
    index: usize,
    parts: &mut Vec<String>,
    f: &mut F,
) -> Result<(), Error>
where
    F: FnMut(&Value, &[String], &mut SchemaScope) -> Result<(), Error>,
{
    match path.get(index) {
        None => f(node, parts, context),
        Some(search) => {
            if let [type_, search_key] = &search.split(':').into_iter().collect::<Vec<&str>>()[..] {
                match *search_key {
                    "*" => match node {
                        Value::Object(ref map) => {
                            for (key, value) in map {
                                context.push_str(*type_, key);

                                parts.push(key.clone());
                                each(value, context, path, index + 1, parts, f)?;
                                parts.pop();

                                context.pop();
                            }

                            Ok(())
                        }
                        _ => Err(Error::NotImplemented),
                    },
                    real_path => {
                        context.push_str(*type_, real_path);

                        if let Some(ref mut found) = node.pointer(&["/", real_path].join("")) {
                            each(found, context, path, index + 1, parts, f)?;
                        }

                        context.pop();

                        Ok(())
                    }
                }
            } else {
                panic!("Incorrect path: {}", search);
            }
        }
    }
}

pub struct ArgumentsExtractor<'a> {
    chars: Chars<'a>,
}

impl<'a> ArgumentsExtractor<'a> {
    pub fn new(command: &'a str) -> Self {
        Self {
            chars: command.chars(),
        }
    }
}

impl<'a> Iterator for ArgumentsExtractor<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut out = String::new();
        let mut escaped = false;
        let mut quote_char = None;
        while let Some(c) = self.chars.next() {
            if escaped {
                out.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if let Some(qc) = quote_char {
                if c == qc {
                    quote_char = None;
                } else {
                    out.push(c);
                }
            } else if c == '\'' || c == '"' {
                quote_char = Some(c);
            } else if c.is_whitespace() {
                if !out.is_empty() {
                    return Some(out);
                } else {
                    continue;
                }
            } else {
                out.push(c);
            }
        }

        if !out.is_empty() {
            Some(out)
        } else {
            None
        }
    }
}

pub fn fill_parameters(phrase: &str, data: (impl Serialize + Clone)) -> Result<String, Error> {
    let chars = phrase.chars();
    let mut result = String::new();

    let mut current = String::new();
    let mut parameter = false;
    for c in chars {
        if c == '%' {
            parameter = !parameter;

            if !current.is_empty() {
                let path = format!("/{}", current.replace(".", "/"));

                if let Some(value) = serde_json::json!(data).pointer(&path) {
                    result.push_str(&match value {
                        Value::String(s) => Ok(s.clone()),
                        Value::Number(n) => Ok(n.to_string()),
                        _ => Err(Error::CannotFillParameters(path)),
                    }?);

                    current.clear();
                } else {
                    return Err(Error::CannotFillParameters(path));
                }
            }

            continue;
        } else if parameter {
            current.push(c)
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_parameters() {
        let given = serde_json::json!({
            "options": {
                "test": "10",
                "num": 2
            }
        });

        let result =
            fill_parameters("some variable %options.test% ok %options.num%", given).unwrap();

        assert_eq!(result, "some variable 10 ok 2".to_string());
    }

    #[test]
    fn test_argument_extractor() {
        let given = "codegen openapi -f - --templates-dir codegen/ --format \"gofmt -w\" --target-dir pkg/client/ -o namespace=testing -o clientName=TestingClient";

        let result: Vec<String> = ArgumentsExtractor::new(given).collect();

        assert_eq!(
            result,
            vec![
                "codegen",
                "openapi",
                "-f",
                "-",
                "--templates-dir",
                "codegen/",
                "--format",
                "gofmt -w",
                "--target-dir",
                "pkg/client/",
                "-o",
                "namespace=testing",
                "-o",
                "clientName=TestingClient"
            ]
            .iter()
            .map(|s| s.clone())
            .collect::<Vec<_>>()
        );
    }
}