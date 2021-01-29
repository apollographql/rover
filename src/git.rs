use rover_client::query::{subgraph, graph};

#[derive(Debug)]
pub(crate) struct GitContext {
    pub branch: Option<String>,
    pub committer: Option<String>,
    pub commit: Option<String>,
    pub message: Option<String>,
    pub remote_url: Option<String>,
}

type GraphPushContextInput = graph::push::push_schema_mutation::GitContextInput;
impl Into<GraphPushContextInput> for GitContext {
    fn into(self) -> GraphPushContextInput {
        GraphPushContextInput {
            branch: self.branch,
            commit: self.commit,
            committer: self.committer,
            remote_url: self.remote_url,
            message: self.message
        }
    }
}

type GraphCheckContextInput = graph::check::check_schema_query::GitContextInput;
impl Into<GraphCheckContextInput> for GitContext {
    fn into(self) -> GraphCheckContextInput {
        GraphCheckContextInput {
            branch: self.branch,
            commit: self.commit,
            committer: self.committer,
            remote_url: self.remote_url,
            message: self.message
        }
    }
}

type SubgraphPushContextInput = subgraph::push::push_partial_schema_mutation::GitContextInput;
impl Into<SubgraphPushContextInput> for GitContext {
    fn into(self) -> SubgraphPushContextInput {
        SubgraphPushContextInput {
            branch: self.branch,
            commit: self.commit,
            committer: self.committer,
            remote_url: self.remote_url,
            message: self.message
        }
    }
}

type SubgraphCheckContextInput = subgraph::check::check_partial_schema_query::GitContextInput;
impl Into<SubgraphCheckContextInput> for GitContext {
    fn into(self) -> SubgraphCheckContextInput {
        SubgraphCheckContextInput {
            branch: self.branch,
            commit: self.commit,
            committer: self.committer,
            remote_url: self.remote_url,
            message: self.message
        }
    }
}

impl GitContext {
    pub fn new() -> Self {
        let git = git_info::get();
        let branch = git.current_branch;
        let commit = git.head.last_commit_hash_short;
        let config_obj = if let Some(mut config) = git.config {
            let remote_url = config.remove("remote.origin.url");
            let user = config.remove("user.name").unwrap_or_else(|| "".to_string());
            let email = config
                .remove("user.email")
                .unwrap_or_else(|| "".to_string());
            let email = if email.is_empty() {
                email
            } else {
                format!("<{}>", email)
            };
            let committer = if user.is_empty() && email.is_empty() {
                None
            } else {
                Some(format!("{} {}", user, email))
            };
            Some((remote_url, committer))
        } else {
            None
        };

        match config_obj {
            Some((remote_url, committer)) => Self {
                branch,
                commit,
                remote_url: GitContext::sanitize_remote_url(remote_url),
                committer,
                message: None,
            },
            None => Self {
                branch,
                commit,
                remote_url: None,
                committer: None,
                message: None,
            },
        }
    }

    /// Parses and sanitizes git remote urls according to the same rules as
    /// defined in apollo-tooling https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L36
    ///
    /// If parsing fails, or if the url doesn't match a valid host, this fn
    /// will return None
    fn sanitize_remote_url(remote: Option<String>) -> Option<String> {
        // try to parse url into git info
        let mut parsed_remote = if let Some(url) = remote {
            match git_url_parse::GitUrl::parse(&url) {
                Ok(parsed_remote) => parsed_remote,
                Err(_err) => return None,
            }
        } else {
            return None;
        };

        // return None for any remote that's not a supported remote
        if let Some(host) = &parsed_remote.host {
            match host.as_str() {
                "github.com" | "gitlab.com" | "bitbucket.org" => {}
                _ => return None,
            }
        } else {
            return None;
        };

        let optional_user = parsed_remote.user.clone();
        parsed_remote = parsed_remote.trim_auth();

        // if the user is "git" we can add back in the user. Otherwise, return
        // the clean remote url
        // this is done previously here:
        // https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L49
        if let Some(user) = &optional_user {
            if user == "git" {
                parsed_remote.user = optional_user;
            }
            Some(parsed_remote.to_string())
        } else {
            Some(parsed_remote.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_user_from_remote_with_only_user() {
        let clean = GitContext::sanitize_remote_url(Some(
            "https://un@bitbucket.org/apollographql/test".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "https://bitbucket.org/apollographql/test".to_string()
        );
    }

    #[test]
    fn does_not_mind_case() {
        let clean = GitContext::sanitize_remote_url(Some(
            "https://un@GITHUB.com/apollographql/test".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "https://github.com/apollographql/test".to_string()
        );
    }

    #[test]
    fn strips_usernames_from_ssh_urls() {
        let clean = GitContext::sanitize_remote_url(Some(
            "ssh://un%401@github.com/apollographql/test".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "ssh://github.com:apollographql/test".to_string()
        );
    }

    #[test]
    fn works_with_special_chars() {
        let clean = GitContext::sanitize_remote_url(Some(
            "https://un:p%40ssw%3Ard@github.com/apollographql/test".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "https://github.com/apollographql/test".to_string()
        );

        let clean = GitContext::sanitize_remote_url(Some(
            "https://un:p%40ssw%3Ard@bitbucket.org/apollographql/test".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "https://bitbucket.org/apollographql/test".to_string()
        );

        let clean = GitContext::sanitize_remote_url(Some(
            "https://un:p%40ssw%3Ard@gitlab.com/apollographql/test".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "https://gitlab.com/apollographql/test".to_string()
        );
    }

    #[test]
    /// preserves `git` user for github
    fn works_with_non_url_github_remotes() {
        let clean = GitContext::sanitize_remote_url(Some(
            "git@github.com:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "git@github.com:apollographql/apollo-tooling.git".to_string()
        );

        let clean = GitContext::sanitize_remote_url(Some(
            "bob@github.com:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "github.com:apollographql/apollo-tooling.git".to_string()
        );
    }

    #[test]
    /// preserves `git` user for bitbucket
    fn works_with_not_url_bitbucket_remotes() {
        let clean = GitContext::sanitize_remote_url(Some(
            "git@bitbucket.org:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "git@bitbucket.org:apollographql/apollo-tooling.git".to_string()
        );

        let clean = GitContext::sanitize_remote_url(Some(
            "bob@bitbucket.org:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "bitbucket.org:apollographql/apollo-tooling.git".to_string()
        );
    }

    #[test]
    /// preserves `git` user for gitlab
    fn works_with_non_url_gitlab_remotes() {
        let clean = GitContext::sanitize_remote_url(Some(
            "git@gitlab.com:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "git@gitlab.com:apollographql/apollo-tooling.git".to_string()
        );

        let clean = GitContext::sanitize_remote_url(Some(
            "bob@gitlab.com:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(
            clean.unwrap(),
            "gitlab.com:apollographql/apollo-tooling.git".to_string()
        );
    }

    #[test]
    fn does_not_allow_remotes_from_unrecognized_providers() {
        let clean = GitContext::sanitize_remote_url(Some(
            "git@lab.com:apollographql/apollo-tooling.git".to_string(),
        ));
        assert_eq!(clean, None);
    }

    #[test]
    fn returns_none_unrecognized_protocol() {
        let clean = GitContext::sanitize_remote_url(Some(
            "git+http://un:p%40sswrd@github.com/apollographql/test".to_string(),
        ));
        assert_eq!(clean, None);
    }
}
