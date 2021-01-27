#[derive(Debug)]
pub(crate) struct GitContext {
    pub branch: Option<String>,
    pub committer: Option<String>,
    pub commit: Option<String>,
    pub message: Option<String>,
    pub remote_url: Option<String>,
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

    fn sanitize_remote_url(remote: Option<String>) -> Option<String> {
      if let Some(url) = remote {
        match git_url_parse::GitUrl::parse(url) {
          Ok(_git_url_info) => {
            Some("".to_string())
          }
          Err(_err) => {
            None
          }
        }
      } else {
        None
      }
    }
}

// export const sanitizeGitRemote = (remote?: string) => {
//   if (!remote) return null;
//   const info = gitUrlParse(remote);

//   // we only support github, gitlab, and bitbucket sources
//   const source = info.source.toLowerCase();
//   if (
//     source !== "github.com" &&
//     source !== "gitlab.com" &&
//     source !== "bitbucket.org"
//   )
//     return null;

//   if (info.user !== "" && info.user !== "git") {
//     info.user = "REDACTED";
//   }
//   info.token = "";

//   // just to make sure that with an unknown `protocol` that stringify doesn't
//   // just print the old, dirty url
//   // https://github.com/IonicaBizau/git-url-parse/blob/0b362b3e3b91a23ae58355fd2160523f0abde5d9/lib/index.js#L216-L217
//   info.href = null;

//   return gitUrlParse.stringify(info);
// };