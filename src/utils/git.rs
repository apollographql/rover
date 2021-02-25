use crate::utils::env::{RoverEnv, RoverEnvKey};
use crate::Result;
use rover_client::query::{graph, subgraph};

use std::env;

use git2::{Reference, Repository};
use git_url_parse::GitUrl;

#[derive(Debug, PartialEq)]
pub struct GitContext {
    pub branch: Option<String>,
    pub committer: Option<String>,
    pub commit: Option<String>,
    pub message: Option<String>,
    pub remote_url: Option<String>,
}

impl GitContext {
    pub fn try_from_rover_env(env: &RoverEnv) -> Result<Self> {
        let repo = Repository::discover(env::current_dir()?);
        match repo {
            Ok(repo) => {
                let head = repo.head().ok();
                Ok(Self {
                    branch: GitContext::get_branch(env, head.as_ref())?,
                    commit: GitContext::get_commit(env, head.as_ref())?,
                    committer: GitContext::get_committer(env, head.as_ref())?,
                    remote_url: GitContext::get_remote_url(env, Some(&repo))?,
                    message: None,
                })
            }
            Err(_) => Ok(Self {
                branch: GitContext::get_branch(env, None)?,
                commit: GitContext::get_commit(env, None)?,
                committer: GitContext::get_committer(env, None)?,
                remote_url: GitContext::get_remote_url(env, None)?,
                message: None,
            }),
        }
    }

    fn get_branch(env: &RoverEnv, head: Option<&Reference>) -> Result<Option<String>> {
        Ok(env.get(RoverEnvKey::VcsBranch)?.or_else(|| {
            let mut branch = None;
            if let Some(head) = head {
                branch = head.shorthand().map(|s| s.to_string())
            }
            branch
        }))
    }

    fn get_commit(env: &RoverEnv, head: Option<&Reference>) -> Result<Option<String>> {
        Ok(env.get(RoverEnvKey::VcsCommit)?.or_else(|| {
            let mut commit = None;
            if let Some(head) = head {
                if let Ok(head_commit) = head.peel_to_commit() {
                    commit = Some(head_commit.id().to_string())
                }
            }
            commit
        }))
    }

    fn get_committer(env: &RoverEnv, head: Option<&Reference>) -> Result<Option<String>> {
        Ok(env.get(RoverEnvKey::VcsCommitter)?.or_else(|| {
            let mut committer = None;
            if let Some(head) = head {
                if let Ok(head_commit) = head.peel_to_commit() {
                    committer = Some(head_commit.committer().to_string())
                }
            }
            committer
        }))
    }

    fn get_remote_url(env: &RoverEnv, repo: Option<&Repository>) -> Result<Option<String>> {
        let remote_url = env.get(RoverEnvKey::VcsRemoteUrl)?.or_else(|| {
            let mut remote_url = None;
            if let Some(repo) = repo {
                if let Ok(remote) = repo.find_remote("origin") {
                    remote_url = remote.url().map(|r| r.to_string())
                }
            }
            remote_url
        });

        Ok(if let Some(remote_url) = remote_url {
            GitContext::sanitize_remote_url(&remote_url)
        } else {
            None
        })
    }

    // Parses and sanitizes git remote urls according to the same rules as
    // defined in apollo-tooling https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L36
    //
    // If parsing fails, or if the url doesn't match a valid host, this fn
    // will return None
    fn sanitize_remote_url(remote_url: &str) -> Option<String> {
        // try to parse url into git info
        let mut parsed_remote_url = match GitUrl::parse(remote_url) {
            Ok(parsed_remote_url) => parsed_remote_url,
            Err(_) => return None,
        };

        // return None for any remote that is not a supported host
        if let Some(host) = &parsed_remote_url.host {
            match host.as_str() {
                "github.com" | "gitlab.com" | "bitbucket.org" => {}
                _ => return None,
            }
        } else {
            return None;
        };

        let optional_user = parsed_remote_url.user.clone();
        parsed_remote_url = parsed_remote_url.trim_auth();

        // if the user is "git" we can add back in the user. Otherwise, return
        // the clean remote url
        // this is done previously here:
        // https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L49
        if let Some(user) = &optional_user {
            if user == "git" {
                parsed_remote_url.user = optional_user;
            }
        };

        Some(parsed_remote_url.to_string())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PKG_NAME;

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

    #[test]
    fn it_can_create_git_context_from_env() {
        let branch = "mybranch".to_string();
        let committer = "test subject number one".to_string();
        let commit = "f84b32caddddfdd9fa87d7ce2140d56eabe805ee".to_string();
        let remote_url = "git@bitbucket.org:roku/theworstremoteintheworld.git".to_string();

        let mut rover_env = RoverEnv::new();
        rover_env.insert(RoverEnvKey::VcsBranch, &branch);
        rover_env.insert(RoverEnvKey::VcsCommitter, &committer);
        rover_env.insert(RoverEnvKey::VcsCommit, &commit);
        rover_env.insert(RoverEnvKey::VcsRemoteUrl, &remote_url);

        let expected_git_context = GitContext {
            branch: Some(branch),
            committer: Some(committer),
            commit: Some(commit),
            message: None,
            remote_url: Some(remote_url),
        };

        let actual_git_context = GitContext::try_from_rover_env(&rover_env)
            .expect("Could not create GitContext from RoverEnv");

        assert_eq!(expected_git_context, actual_git_context);
    }

    #[test]
    fn it_can_create_git_context_committ_committer_remote_url() {
        let git_context =
            GitContext::try_from_rover_env(&RoverEnv::new()).expect("Could not create git context");

        assert!(git_context.branch.is_some());
        assert!(git_context.committer.is_some());

        if let Some(commit) = git_context.commit {
            assert_eq!(commit.len(), 40);
        } else {
            panic!("Could not find the commit hash");
        }

        assert!(git_context.message.is_none());

        if let Some(remote_url) = git_context.remote_url {
            assert!(remote_url.contains(PKG_NAME));
        } else {
            panic!("GitContext could not find the remote url");
        }
    }
}
