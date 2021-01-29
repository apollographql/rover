use rover_client::query::{graph, subgraph};
#[derive(Debug)]
pub struct GitContext {
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
            message: self.message,
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
            message: self.message,
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
            message: self.message,
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
            message: self.message,
        }
    }
}

impl GitContext {
    pub fn new(
        override_branch: Option<String>,
        override_committer: Option<String>,
        override_commit: Option<String>,
        override_remote_url: Option<String>,
    ) -> Self {
        let git = git_info::get();

        let branch = if override_branch.is_none() {
            git.current_branch
        } else {
            override_branch
        };
        let commit = if override_commit.is_none() {
            git.head.last_commit_hash_short
        } else {
            override_commit
        };

        let mut remote_url = override_remote_url;
        let mut committer = override_committer;

        // if both remote_url and committer have values, we don't need to
        // worry about executing this block
        if remote_url.is_none() || committer.is_none() {
            if let Some(mut config) = git.config {
                // use the local git remote url if not provided in env var
                remote_url = if remote_url.is_none() {
                    config.remove("remote.origin.url")
                } else {
                    remote_url
                };

                // if the committer from env is None, build from git info
                committer = if committer.is_none() {
                    let user = config.remove("user.name").unwrap_or_else(|| "".to_string());
                    let email = config
                        .remove("user.email")
                        .unwrap_or_else(|| "".to_string());
                    let email = if email.is_empty() {
                        email
                    } else {
                        format!("<{}>", email)
                    };

                    // build final formatted committer
                    if user.is_empty() && email.is_empty() {
                        None
                    } else {
                        Some(format!("{} {}", user, email))
                    }
                } else {
                    committer
                };
            };
        };

        let remote_url = if let Some(remote) = remote_url {
            GitContext::sanitize_remote_url(&remote)
        } else {
            None
        };

        Self {
            branch,
            commit,
            remote_url,
            committer,
            message: None,
        }
    }

    /// Parses and sanitizes git remote urls according to the same rules as
    /// defined in apollo-tooling https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L36
    ///
    /// If parsing fails, or if the url doesn't match a valid host, this fn
    /// will return None
    fn sanitize_remote_url(remote: &str) -> Option<String> {
        // try to parse url into git info
        let mut parsed_remote = match git_url_parse::GitUrl::parse(remote) {
            Ok(parsed_remote) => parsed_remote,
            Err(_err) => return None,
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
        let clean = GitContext::sanitize_remote_url("https://un@bitbucket.org/apollographql/test");
        assert_eq!(
            clean.unwrap(),
            "https://bitbucket.org/apollographql/test".to_string()
        );
    }

    #[test]
    fn does_not_mind_case() {
        let clean = GitContext::sanitize_remote_url("https://un@GITHUB.com/apollographql/test");
        assert_eq!(
            clean.unwrap(),
            "https://github.com/apollographql/test".to_string()
        );
    }

    #[test]
    fn strips_usernames_from_ssh_urls() {
        let clean = GitContext::sanitize_remote_url("ssh://un%401@github.com/apollographql/test");
        assert_eq!(
            clean.unwrap(),
            "ssh://github.com:apollographql/test".to_string()
        );
    }

    #[test]
    fn works_with_special_chars() {
        let clean = GitContext::sanitize_remote_url(
            "https://un:p%40ssw%3Ard@github.com/apollographql/test",
        );
        assert_eq!(
            clean.unwrap(),
            "https://github.com/apollographql/test".to_string()
        );

        let clean = GitContext::sanitize_remote_url(
            "https://un:p%40ssw%3Ard@bitbucket.org/apollographql/test",
        );
        assert_eq!(
            clean.unwrap(),
            "https://bitbucket.org/apollographql/test".to_string()
        );

        let clean = GitContext::sanitize_remote_url(
            "https://un:p%40ssw%3Ard@gitlab.com/apollographql/test",
        );
        assert_eq!(
            clean.unwrap(),
            "https://gitlab.com/apollographql/test".to_string()
        );
    }

    #[test]
    /// preserves `git` user for github
    fn works_with_non_url_github_remotes() {
        let clean =
            GitContext::sanitize_remote_url("git@github.com:apollographql/apollo-tooling.git");
        assert_eq!(
            clean.unwrap(),
            "git@github.com:apollographql/apollo-tooling.git".to_string()
        );

        let clean =
            GitContext::sanitize_remote_url("bob@github.com:apollographql/apollo-tooling.git");
        assert_eq!(
            clean.unwrap(),
            "github.com:apollographql/apollo-tooling.git".to_string()
        );
    }

    #[test]
    /// preserves `git` user for bitbucket
    fn works_with_not_url_bitbucket_remotes() {
        let clean =
            GitContext::sanitize_remote_url("git@bitbucket.org:apollographql/apollo-tooling.git");
        assert_eq!(
            clean.unwrap(),
            "git@bitbucket.org:apollographql/apollo-tooling.git".to_string()
        );

        let clean =
            GitContext::sanitize_remote_url("bob@bitbucket.org:apollographql/apollo-tooling.git");
        assert_eq!(
            clean.unwrap(),
            "bitbucket.org:apollographql/apollo-tooling.git".to_string()
        );
    }

    #[test]
    /// preserves `git` user for gitlab
    fn works_with_non_url_gitlab_remotes() {
        let clean =
            GitContext::sanitize_remote_url("git@gitlab.com:apollographql/apollo-tooling.git");
        assert_eq!(
            clean.unwrap(),
            "git@gitlab.com:apollographql/apollo-tooling.git".to_string()
        );

        let clean =
            GitContext::sanitize_remote_url("bob@gitlab.com:apollographql/apollo-tooling.git");
        assert_eq!(
            clean.unwrap(),
            "gitlab.com:apollographql/apollo-tooling.git".to_string()
        );
    }

    #[test]
    fn does_not_allow_remotes_from_unrecognized_providers() {
        let clean = GitContext::sanitize_remote_url("git@lab.com:apollographql/apollo-tooling.git");
        assert_eq!(clean, None);
    }

    #[test]
    fn returns_none_unrecognized_protocol() {
        let clean = GitContext::sanitize_remote_url(
            "git+http://un:p%40sswrd@github.com/apollographql/test",
        );
        assert_eq!(clean, None);
    }
}
