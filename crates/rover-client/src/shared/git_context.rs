use std::env;

use git2::{Reference, Repository};
use serde::Serialize;
use url::form_urlencoded;

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct GitContext {
    pub branch: Option<String>,
    pub author: Option<String>,
    pub commit: Option<String>,
    pub remote_url: Option<String>,
}

impl GitContext {
    pub fn new_with_override(override_git_context: GitContext) -> Self {
        let repo = GitContext::get_repo();

        let mut remote_url = override_git_context.remote_url;

        if let Some(repo) = repo {
            remote_url = remote_url.or_else(|| GitContext::get_remote_url(&repo));
            if let Ok(head) = repo.head() {
                let branch = override_git_context
                    .branch
                    .or_else(|| GitContext::get_branch(&head));

                let author = override_git_context
                    .author
                    .or_else(|| GitContext::get_author(&head));

                let commit = override_git_context
                    .commit
                    .or_else(|| GitContext::get_commit(&head));

                return GitContext {
                    branch,
                    author,
                    commit,
                    remote_url,
                };
            }
        }

        GitContext {
            branch: override_git_context.branch,
            author: override_git_context.author,
            commit: override_git_context.commit,
            remote_url,
        }
    }

    fn get_repo() -> Option<Repository> {
        env::current_dir()
            .map(|d| Repository::discover(d).ok())
            .ok()
            .flatten()
    }

    fn get_branch(head: &Reference) -> Option<String> {
        head.shorthand()
            .inspect_err(|e| tracing::error!("failed to read git branch shorthand: {e}"))
            .ok()
            .map(|s| s.to_string())
    }

    fn get_commit(head: &Reference) -> Option<String> {
        if let Ok(head_commit) = head.peel_to_commit() {
            Some(head_commit.id().to_string())
        } else {
            None
        }
    }

    fn get_author(head: &Reference) -> Option<String> {
        if let Ok(head_commit) = head.peel_to_commit() {
            Some(head_commit.author().to_string())
        } else {
            None
        }
    }

    fn get_remote_url(repo: &Repository) -> Option<String> {
        let remote_url = if let Ok(remote) = repo.find_remote("origin") {
            remote
                .url()
                .inspect_err(|e| tracing::error!("failed to read git remote url: {e}"))
                .ok()
                .map(|r| r.to_string())
        } else {
            None
        };
        remote_url.and_then(|r| GitContext::sanitize_remote_url(&r))
    }

    // Parses and sanitizes git remote urls according to the same rules as
    // defined in apollo-tooling https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L36
    //
    // Strips any username and password information from a git remote
    //
    // If parsing fails, or if the url doesn't match a valid host, this fn
    // will return None
    fn sanitize_remote_url(remote_url: &str) -> Option<String> {
        let remote_url = remote_url.to_lowercase();
        let mut parsed_remote_url = parse_git_remote(&remote_url)?;

        // return None for any remote that does not have a host
        parsed_remote_url.host()?;

        // if the user is "git" we keep it. Otherwise, strip the user from the remote url
        // this is done previously here:
        // https://github.com/apollographql/apollo-tooling/blob/fd642ab59620cd836651dcab4c3ecbcbcca3f780/packages/apollo/src/git.ts#L49
        if parsed_remote_url.user() != Some("git") {
            parsed_remote_url.set_user(None);
        }
        parsed_remote_url.set_password(None);

        Some(parsed_remote_url.to_bstring().to_string())
    }
}

impl Default for GitContext {
    fn default() -> Self {
        Self::new_with_override(Self {
            author: None,
            branch: None,
            commit: None,
            remote_url: None,
        })
    }
}

fn parse_git_remote(remote_url: &str) -> Option<gix_url::Url> {
    // Removes encoded `@`s from usernames and passwords
    let remote_url = remote_url.replace("%40", "");
    // Decodes url components
    let mut decoded = form_urlencoded::parse(remote_url.as_bytes());
    let decoded = decoded.next()?;
    // Should not have anything for url component, only valid in query pairs
    if !decoded.1.is_empty() {
        return None;
    }
    gix_url::parse(&*decoded.0).ok()
}

#[cfg(test)]
#[expect(clippy::panic)]
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
            "ssh://github.com/apollographql/test".to_string()
        );
    }

    #[test]
    fn keeps_git_user_in_ssh_urls() {
        let clean = GitContext::sanitize_remote_url("ssh://git:secret@github.com/org/repo.git");
        assert_eq!(clean, Some("ssh://git@github.com/org/repo.git".to_string()));
    }

    #[test]
    fn rejects_remotes_without_a_host() {
        assert_eq!(GitContext::sanitize_remote_url("file:///tmp/repo"), None);
        assert_eq!(GitContext::sanitize_remote_url("/tmp/repo"), None);
        assert_eq!(GitContext::sanitize_remote_url("../repo"), None);
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
    fn it_can_create_git_context_from_env() {
        let branch = "mybranch".to_string();
        let author = "test subject number one".to_string();
        let commit = "f84b32caddddfdd9fa87d7ce2140d56eabe805ee".to_string();
        let remote_url = "git@bitbucket.org:roku/theworstremoteintheworld.git".to_string();

        let override_git_context = GitContext {
            branch: Some(branch),
            author: Some(author),
            commit: Some(commit),
            remote_url: Some(remote_url),
        };

        let actual_git_context = GitContext::new_with_override(override_git_context.clone());

        assert_eq!(override_git_context, actual_git_context);
    }

    #[test]
    fn it_can_create_git_context_commit_author_remote_url() {
        let git_context = GitContext::default();

        assert!(git_context.branch.is_some());
        assert!(git_context.author.is_some());

        if let Some(commit) = git_context.commit {
            assert_eq!(commit.len(), 40);
        } else {
            panic!("Could not find the commit hash");
        }

        if let Some(remote_url) = git_context.remote_url {
            assert!(remote_url.contains("rover"));
        } else {
            panic!("GitContext could not find the remote url");
        }
    }

    #[test]
    // regression test for https://github.com/apollographql/rover/issues/670
    fn it_does_not_panic_on_remote_urls_with_no_apparent_owner() {
        let clean = GitContext::sanitize_remote_url("ssh://user@github.com/repo-name");
        assert_eq!(clean, Some("ssh://github.com/repo-name".to_string()));
    }
}
