use super::word::{pluralize, singularize};

use crate::error::Error;
use inflector::Inflector;
use regex::Regex;

pub struct Endpoint {
    original: String,
    version: Option<String>,
    method: String,
    resources: Vec<String>,
    identifiers: Vec<String>,
}

impl Endpoint {
    pub fn new(method: String, original_path: String) -> Result<Endpoint, Error> {
        lazy_static::lazy_static! {
            static ref METHOD: Regex =
                Regex::new("^(get|head|post|put|delete|connect|options|trace|patch)$").unwrap();
            static ref VERSION: Regex = Regex::new("^v([0-9]+)$").unwrap();
        }

        let path = original_path
            .trim_matches('/')
            .trim_matches('_')
            .to_string();

        if !METHOD.is_match(&method) || path.is_empty() {
            return Err(Error::EndpointValidation { method, path });
        }

        let mut parts = path.split('/').collect::<Vec<&str>>();
        let mut version = None;

        if VERSION.is_match(parts.first().unwrap()) {
            version = Some(parts.first().unwrap().to_string());
            parts = parts.drain(1..).collect(); // shift vectors
        }

        // todo: optimize
        let resources: Vec<String> = parts
            .clone()
            .into_iter()
            .filter(|s| !s.starts_with('{'))
            .map(|s| s.to_string())
            .collect();
        let identifiers: Vec<String> = parts
            .into_iter()
            .filter(|s| s.starts_with('{'))
            .map(|s| s.to_string())
            .collect();

        Ok(Endpoint {
            original: original_path,
            version,
            method,
            resources,
            identifiers,
        })
    }

    pub fn get_operation_id(&self, resource_method_version: bool) -> String {
        let mut parts: Vec<String> = vec![];

        if let Some(v) = self.version.clone() {
            parts.push(v);
        }

        parts.push(
            match self.method.as_str() {
                "get" => {
                    if !self.original.ends_with('}') {
                        "list"
                    } else {
                        "get"
                    }
                }
                "post" => "create",
                "patch" => "update",
                m => m,
            }
            .to_string(),
        );

        let mut resources: Vec<String> = vec![];
        for (i, resource) in self.resources.iter().enumerate() {
            let processed = resource.clone().to_camel_case();

            resources.push(
                {
                    if i < self.identifiers.len() {
                        // has identifier
                        singularize(processed)
                    } else {
                        match self.method.as_str() {
                            "post" => singularize(processed),
                            "get" => processed,
                            _ => pluralize(processed),
                        }
                    }
                }
                .to_string(),
            );
        }

        if !resource_method_version {
            parts.append(&mut resources)
        } else {
            parts.reverse();
            resources.append(&mut parts);
            parts = resources;
        };

        // camelcase
        parts
            .into_iter()
            .collect::<Vec<String>>()
            .join(" ")
            .to_camel_case()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case( "get".to_string(), "users/{id}".to_string(), "getUser".to_string(); "endpoint name test 1" )]
    #[test_case( "post".to_string(), "users/{id}/groups".to_string(), "createUserGroup".to_string(); "endpoint name test 2" )]
    #[test_case( "get".to_string(), "users/{id}/groups".to_string(), "listUserGroups".to_string(); "endpoint name test 3" )]
    #[test_case( "patch".to_string(), "users/{id}/groups".to_string(), "updateUserGroups".to_string(); "endpoint name test 4" )]
    #[test_case( "patch".to_string(), "users/{id}/groups/{id}".to_string(), "updateUserGroup".to_string(); "endpoint name test 5" )]
    #[test_case( "get".to_string(), "users/{id}/groups/{id}".to_string(), "getUserGroup".to_string(); "endpoint name test 6" )]
    #[test_case( "get".to_string(), "users".to_string(), "listUsers".to_string(); "endpoint name test 7" )]
    #[test_case( "get".to_string(), "users/{id}".to_string(), "getUser".to_string(); "endpoint name test 8" )]
    #[test_case( "get".to_string(), "v2/users".to_string(), "v2ListUsers".to_string(); "endpoint name test 9" )]
    #[test_case( "get".to_string(), "v2/users/{id}".to_string(), "v2GetUser".to_string(); "endpoint name test 10" )]
    #[test_case( "get".to_string(), "v1/users/{id}/status".to_string(), "v1ListUserStatus".to_string(); "endpoint name test 11" )]
    #[test_case( "post".to_string(), "users/{id}/groups".to_string(), "createUserGroup".to_string(); "endpoint name test 12" )]
    #[test_case( "get".to_string(), "user-groups/{id}".to_string(), "getUserGroup".to_string(); "endpoint name test 13" )]
    #[test_case( "get".to_string(), "v1/users/{id}/statuses".to_string(), "v1ListUserStatuses".to_string(); "endpoint name test 14" )]
    fn test_operation_name(method: String, path: String, expected: String) {
        assert_eq!(
            Endpoint::new(method, path).unwrap().get_operation_id(false),
            expected
        );
    }

    #[test_case( "get".to_string(), "/api/xyz/v1/namespaces".to_string(), "listApiXyzV1Namespaces".to_string(); "endpoint name non restful endpoint 1" )]
    #[test_case( "get".to_string(), "/api/xyz/v1/namespaces/{namespaceId}".to_string(), "getApiXyzV1Namespaces".to_string(); "endpoint name non restful endpoint 2" )]
    fn test_operation_name2(method: String, path: String, expected: String) {
        assert_eq!(
            Endpoint::new(method, path).unwrap().get_operation_id(false),
            expected
        );
    }

    #[test_case( "get".to_string(), "user-groups/{id}".to_string(), "userGroupGet".to_string(); "endpoint name reverse test 1" )]
    #[test_case( "get".to_string(), "v1/users/{id}/statuses".to_string(), "userStatusesListV1".to_string(); "endpoint name reverse test 2" )]
    fn test_operation_name_reverse(method: String, path: String, expected: String) {
        assert_eq!(
            Endpoint::new(method, path).unwrap().get_operation_id(true),
            expected
        );
    }
}
