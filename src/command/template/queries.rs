#![allow(clippy::all, warnings)]
pub struct ListTemplatesForLanguage;
pub mod list_templates_for_language {
    #![allow(dead_code)]
    use std::result::Result;
    pub const OPERATION_NAME: &str = "ListTemplatesForLanguage";
    pub const QUERY: &str = "query ListTemplatesForLanguage($language: Language) {\n    templates(language: $language) {\n        id\n        name\n        description\n        repoUrl\n        language\n    }\n}\n\nquery GetTemplateById($id: ID!) {\n    template(id: $id) {\n        downloadUrl\n    }\n}\n\nquery GetTemplatesForLanguage($language: Language) {\n    templates(language: $language) {\n        id\n        name\n        downloadUrl\n    }\n}" ;
    use super::*;
    use serde::{Deserialize, Serialize};
    #[allow(dead_code)]
    type Boolean = bool;
    #[allow(dead_code)]
    type Float = f64;
    #[allow(dead_code)]
    type Int = i64;
    #[allow(dead_code)]
    type ID = String;
    type Url = crate::command::template::custom_scalars::Url;
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum Language {
        C_SHARP,
        GO,
        JAVA,
        JAVASCRIPT,
        KOTLIN,
        PYTHON,
        RUST,
        TYPESCRIPT,
        Other(String),
    }
    impl ::serde::Serialize for Language {
        fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
            ser.serialize_str(match *self {
                Language::C_SHARP => "C_SHARP",
                Language::GO => "GO",
                Language::JAVA => "JAVA",
                Language::JAVASCRIPT => "JAVASCRIPT",
                Language::KOTLIN => "KOTLIN",
                Language::PYTHON => "PYTHON",
                Language::RUST => "RUST",
                Language::TYPESCRIPT => "TYPESCRIPT",
                Language::Other(ref s) => &s,
            })
        }
    }
    impl<'de> ::serde::Deserialize<'de> for Language {
        fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let s: String = ::serde::Deserialize::deserialize(deserializer)?;
            match s.as_str() {
                "C_SHARP" => Ok(Language::C_SHARP),
                "GO" => Ok(Language::GO),
                "JAVA" => Ok(Language::JAVA),
                "JAVASCRIPT" => Ok(Language::JAVASCRIPT),
                "KOTLIN" => Ok(Language::KOTLIN),
                "PYTHON" => Ok(Language::PYTHON),
                "RUST" => Ok(Language::RUST),
                "TYPESCRIPT" => Ok(Language::TYPESCRIPT),
                _ => Ok(Language::Other(s)),
            }
        }
    }
    #[derive(Serialize)]
    pub struct Variables {
        pub language: Option<Language>,
    }
    impl Variables {}
    #[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
    pub struct ResponseData {
        pub templates: Vec<ListTemplatesForLanguageTemplates>,
    }
    #[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
    pub struct ListTemplatesForLanguageTemplates {
        pub id: ID,
        pub name: String,
        pub description: String,
        #[serde(rename = "repoUrl")]
        pub repo_url: Url,
        pub language: Language,
    }
}
impl graphql_client::GraphQLQuery for ListTemplatesForLanguage {
    type Variables = list_templates_for_language::Variables;
    type ResponseData = list_templates_for_language::ResponseData;
    fn build_query(variables: Self::Variables) -> ::graphql_client::QueryBody<Self::Variables> {
        graphql_client::QueryBody {
            variables,
            query: list_templates_for_language::QUERY,
            operation_name: list_templates_for_language::OPERATION_NAME,
        }
    }
}
pub struct GetTemplateById;
pub mod get_template_by_id {
    #![allow(dead_code)]
    use std::result::Result;
    pub const OPERATION_NAME: &str = "GetTemplateById";
    pub const QUERY: &str = "query ListTemplatesForLanguage($language: Language) {\n    templates(language: $language) {\n        id\n        name\n        description\n        repoUrl\n        language\n    }\n}\n\nquery GetTemplateById($id: ID!) {\n    template(id: $id) {\n        downloadUrl\n    }\n}\n\nquery GetTemplatesForLanguage($language: Language) {\n    templates(language: $language) {\n        id\n        name\n        downloadUrl\n    }\n}" ;
    use super::*;
    use serde::{Deserialize, Serialize};
    #[allow(dead_code)]
    type Boolean = bool;
    #[allow(dead_code)]
    type Float = f64;
    #[allow(dead_code)]
    type Int = i64;
    #[allow(dead_code)]
    type ID = String;
    type Url = crate::command::template::custom_scalars::Url;
    #[derive(Serialize)]
    pub struct Variables {
        pub id: ID,
    }
    impl Variables {}
    #[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
    pub struct ResponseData {
        pub template: Option<GetTemplateByIdTemplate>,
    }
    #[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
    pub struct GetTemplateByIdTemplate {
        #[serde(rename = "downloadUrl")]
        pub download_url: Url,
    }
}
impl graphql_client::GraphQLQuery for GetTemplateById {
    type Variables = get_template_by_id::Variables;
    type ResponseData = get_template_by_id::ResponseData;
    fn build_query(variables: Self::Variables) -> ::graphql_client::QueryBody<Self::Variables> {
        graphql_client::QueryBody {
            variables,
            query: get_template_by_id::QUERY,
            operation_name: get_template_by_id::OPERATION_NAME,
        }
    }
}
pub struct GetTemplatesForLanguage;
pub mod get_templates_for_language {
    #![allow(dead_code)]
    use std::result::Result;
    pub const OPERATION_NAME: &str = "GetTemplatesForLanguage";
    pub const QUERY: &str = "query ListTemplatesForLanguage($language: Language) {\n    templates(language: $language) {\n        id\n        name\n        description\n        repoUrl\n        language\n    }\n}\n\nquery GetTemplateById($id: ID!) {\n    template(id: $id) {\n        downloadUrl\n    }\n}\n\nquery GetTemplatesForLanguage($language: Language) {\n    templates(language: $language) {\n        id\n        name\n        downloadUrl\n    }\n}" ;
    use super::*;
    use serde::{Deserialize, Serialize};
    #[allow(dead_code)]
    type Boolean = bool;
    #[allow(dead_code)]
    type Float = f64;
    #[allow(dead_code)]
    type Int = i64;
    #[allow(dead_code)]
    type ID = String;
    type Url = crate::command::template::custom_scalars::Url;
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum Language {
        C_SHARP,
        GO,
        JAVA,
        JAVASCRIPT,
        KOTLIN,
        PYTHON,
        RUST,
        TYPESCRIPT,
        Other(String),
    }
    impl ::serde::Serialize for Language {
        fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
            ser.serialize_str(match *self {
                Language::C_SHARP => "C_SHARP",
                Language::GO => "GO",
                Language::JAVA => "JAVA",
                Language::JAVASCRIPT => "JAVASCRIPT",
                Language::KOTLIN => "KOTLIN",
                Language::PYTHON => "PYTHON",
                Language::RUST => "RUST",
                Language::TYPESCRIPT => "TYPESCRIPT",
                Language::Other(ref s) => &s,
            })
        }
    }
    impl<'de> ::serde::Deserialize<'de> for Language {
        fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let s: String = ::serde::Deserialize::deserialize(deserializer)?;
            match s.as_str() {
                "C_SHARP" => Ok(Language::C_SHARP),
                "GO" => Ok(Language::GO),
                "JAVA" => Ok(Language::JAVA),
                "JAVASCRIPT" => Ok(Language::JAVASCRIPT),
                "KOTLIN" => Ok(Language::KOTLIN),
                "PYTHON" => Ok(Language::PYTHON),
                "RUST" => Ok(Language::RUST),
                "TYPESCRIPT" => Ok(Language::TYPESCRIPT),
                _ => Ok(Language::Other(s)),
            }
        }
    }
    #[derive(Serialize)]
    pub struct Variables {
        pub language: Option<Language>,
    }
    impl Variables {}
    #[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
    pub struct ResponseData {
        pub templates: Vec<GetTemplatesForLanguageTemplates>,
    }
    #[derive(Deserialize, Debug, Serialize, PartialEq, Eq, Clone)]
    pub struct GetTemplatesForLanguageTemplates {
        pub id: ID,
        pub name: String,
        #[serde(rename = "downloadUrl")]
        pub download_url: Url,
    }
}
impl graphql_client::GraphQLQuery for GetTemplatesForLanguage {
    type Variables = get_templates_for_language::Variables;
    type ResponseData = get_templates_for_language::ResponseData;
    fn build_query(variables: Self::Variables) -> ::graphql_client::QueryBody<Self::Variables> {
        graphql_client::QueryBody {
            variables,
            query: get_templates_for_language::QUERY,
            operation_name: get_templates_for_language::OPERATION_NAME,
        }
    }
}
