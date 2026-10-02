pub(crate) mod error;
pub(crate) mod output;

use std::io::{self, IsTerminal};

use clap::Parser;
use error::GrantsRevokeError;
use output::{ClientOutcome, RevokeSweepOutput, SweptClient};
use rover_client::{
    RoverClientError,
    operations::{
        api_key::pair_list::{
            LIST_PAIRS_ATTEMPT_TIMEOUT, ListOAuthClients, ListOAuthClientsError,
            ListOAuthClientsInput, ListOAuthClientsResponse,
        },
        auth::{
            org_membership::{ORG_MEMBERSHIP_ATTEMPT_TIMEOUT, OrgMembership, OrgMembershipInput},
            revoke_user_grants::{
                REVOKE_USER_GRANTS_ATTEMPT_TIMEOUT, RevokeUserGrants, RevokeUserGrantsInput,
            },
        },
    },
};
use rover_print::{
    print::Print,
    style::{Style, StyledText},
};
use serde::Serialize;
use tower::{Service, ServiceExt};

use crate::{
    RoverError, RoverOutput, RoverResult, command::auth::OauthConfig, options::ProfileOpt,
    utils::client::StudioClientConfig,
};

/// FR47-FR49: the sweep is the only accepted form. `--org`, `--user`, and `--all` are each
/// required, so every other combination - including a bare grant ID, which has no positional
/// to land in - is a usage error, raised before any request.
#[derive(Debug, Serialize, Parser)]
pub(crate) struct Revoke {
    /// The organization whose OAuth clients to revoke under
    #[clap(long = "org", value_name = "ORGANIZATION_ID", required = true)]
    organization_id: String,

    /// The user whose grants to revoke
    #[clap(long = "user", value_name = "USER_ID", required = true)]
    user_id: String,

    /// Revoke every grant the user holds, under Rover's OAuth client and every
    /// client-credential pair in the organization
    #[clap(long, required = true)]
    all: bool,

    /// Skip the confirmation prompt
    #[clap(long)]
    confirm: bool,
}

/// How the sweep gets its go-ahead (FR57, FR59).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirmation {
    /// `--confirm` was passed.
    Given,
    /// Ask on the terminal.
    Ask,
    /// No terminal to ask on, or `--format json` - fail rather than wait (FR59).
    Unavailable,
}

impl Revoke {
    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        oauth_config: &OauthConfig,
        profile: &ProfileOpt,
        json: bool,
    ) -> RoverResult<RoverOutput> {
        let confirmation = if self.confirm {
            Confirmation::Given
        } else if json || !io::stdin().is_terminal() {
            Confirmation::Unavailable
        } else {
            Confirmation::Ask
        };
        let client = client_config.get_authenticated_client(profile)?;
        let services = Services {
            pairs: ListOAuthClients::new(
                client.studio_graphql_service_with_attempt_timeout(LIST_PAIRS_ATTEMPT_TIMEOUT)?,
            ),
            membership: OrgMembership::new(
                client
                    .studio_graphql_service_with_attempt_timeout(ORG_MEMBERSHIP_ATTEMPT_TIMEOUT)?,
            ),
            revoke: RevokeUserGrants::new(
                client.studio_graphql_service_with_attempt_timeout(
                    REVOKE_USER_GRANTS_ATTEMPT_TIMEOUT,
                )?,
            ),
        };
        let target = Target {
            organization_id: &self.organization_id,
            user_id: &self.user_id,
            rover_client_id: &oauth_config.client_id,
        };
        let output = sweep(
            target,
            confirmation,
            services,
            &rover_print::print::stderr::default(),
            rover_std::prompt::prompt_confirm_default_no,
        )
        .await?;
        Ok(RoverOutput::CliOutput(Box::new(output)))
    }
}

/// Who and where to sweep.
#[derive(Debug, Clone, Copy)]
struct Target<'a> {
    organization_id: &'a str,
    user_id: &'a str,
    /// The effective `APOLLO_OAUTH_CLIENT_ID` (FR67).
    rover_client_id: &'a str,
}

/// The Platform API calls a sweep makes, injected so tests can supply mocks. Each production
/// service is composed under `StudioClient`'s retry, with a per-attempt timeout inside it - all
/// three are safe to retry (two reads, and an idempotent revoke, FR64).
struct Services<L, M, R> {
    pairs: L,
    membership: M,
    revoke: R,
}

/// FR55-FR66: the per-user sweep. Enumerates every pair, checks membership, asks for
/// confirmation, then revokes under Rover's client and every pair, attempting each one even
/// after another fails.
async fn sweep<L, M, R, P, C>(
    target: Target<'_>,
    confirmation: Confirmation,
    mut services: Services<L, M, R>,
    stderr: &P,
    confirm: C,
) -> RoverResult<RevokeSweepOutput>
where
    L: Service<
            ListOAuthClientsInput,
            Response = ListOAuthClientsResponse,
            Error = ListOAuthClientsError,
        >,
    M: Service<OrgMembershipInput, Response = Option<bool>, Error = RoverClientError>,
    R: Service<RevokeUserGrantsInput, Response = (), Error = RoverClientError>,
    P: Print,
    C: FnOnce(&str) -> io::Result<bool>,
{
    let Target {
        organization_id,
        user_id,
        rover_client_id,
    } = target;

    // FR59: nothing to ask on, so fail before making any request at all.
    if confirmation == Confirmation::Unavailable {
        return Err(GrantsRevokeError::ConfirmationRequired {
            user_id: user_id.to_string(),
        }
        .into());
    }

    // FR56: every pair, through every page, before anything is revoked.
    let enumeration_failed = |err: ListOAuthClientsError| GrantsRevokeError::PairEnumeration {
        organization_id: organization_id.to_string(),
        reason: err.to_string(),
    };
    let pairs = services
        .pairs
        .ready()
        .await
        .map_err(enumeration_failed)?
        .call(
            // No cap (FR56, spec §6): the sweep revokes based on what it enumerates, so it must
            // see every pair. `pair_list`'s own `NoProgress`/`MissingCursor` guards are what stop
            // a misbehaving server from paging forever.
            ListOAuthClientsInput::builder()
                .organization_id(organization_id)
                .limit(usize::MAX)
                .build(),
        )
        .await
        .map_err(enumeration_failed)?;
    // Unreachable with no cap, but a list that says there's more must never be swept as if it
    // were the whole organization.
    if pairs.next_after.is_some() {
        return Err(GrantsRevokeError::PairEnumeration {
            organization_id: organization_id.to_string(),
            reason: "the listing stopped before its last page".to_string(),
        }
        .into());
    }

    // FR60: membership decides only the warning. A lookup that fails - including a caller who
    // may revoke grants but not read the member list - means Rover can't tell, which mustn't
    // stop the sweep it isn't needed for.
    let user_is_member = match services.membership.ready().await {
        Ok(membership) => {
            membership
                .call(
                    OrgMembershipInput::builder()
                        .organization_id(organization_id)
                        .user_id(user_id)
                        .build(),
                )
                .await
        }
        Err(err) => Err(err),
    }
    .unwrap_or_else(|err| {
        tracing::debug!("couldn't check `{user_id}`'s membership of `{organization_id}`: {err}");
        None
    });
    if user_is_member == Some(false) {
        stderr.print_line(&[
            StyledText::new(Style::Warning, "Warning:"),
            StyledText::plain(format!(
                " `{user_id}` isn't a current member of organization `{organization_id}`. Rover \
                will still revoke any grants they hold under the clients below."
            )),
        ]);
    }

    let clients: Vec<SweptClient> = std::iter::once(SweptClient::rover(rover_client_id))
        .chain(pairs.pairs.iter().map(SweptClient::pair))
        .collect();

    // FR57/FR58.
    if confirmation == Confirmation::Ask {
        let listed: String = clients
            .iter()
            .map(|client| format!("\n  - {}", client.prompt_label()))
            .collect();
        let prompt = format!(
            "This revokes every grant `{user_id}` holds under these OAuth clients in \
            organization `{organization_id}`:{listed}\nIt doesn't revoke access tokens they \
            already hold, and doesn't stop them logging in again.\nRevoke?"
        );
        if !confirm(&prompt).map_err(RoverError::new)? {
            return Ok(RevokeSweepOutput {
                organization_id: organization_id.to_string(),
                user_id: user_id.to_string(),
                user_is_member,
                clients: Vec::new(),
                cancelled: true,
            });
        }
    }

    // FR61: every client is attempted, whatever happened under the one before.
    let mut outcomes = Vec::with_capacity(clients.len());
    let mut every_failure_was_a_refusal = true;
    for client in clients {
        let input = RevokeUserGrantsInput::builder()
            .organization_id(organization_id)
            .client_id(client.client_id.clone())
            .user_id(user_id)
            .build();
        let result = match services.revoke.ready().await {
            Ok(revoke) => revoke.call(input).await,
            Err(err) => Err(err),
        };
        let error = match result {
            Ok(()) => None,
            Err(err) => {
                every_failure_was_a_refusal &=
                    matches!(err, RoverClientError::GrantPermissionDenied { .. });
                Some(err.to_string())
            }
        };
        outcomes.push(ClientOutcome { client, error });
    }

    let output = RevokeSweepOutput {
        organization_id: organization_id.to_string(),
        user_id: user_id.to_string(),
        user_is_member,
        clients: outcomes,
        cancelled: false,
    };
    match output.failed().count() {
        0 => Ok(output),
        // FR73: refused under every client is a permission problem, not a partial sweep.
        failed if failed == output.clients.len() && every_failure_was_a_refusal => {
            Err(RoverClientError::GrantPermissionDenied {
                organization_id: organization_id.to_string(),
            }
            .into())
        }
        // FR63.
        _ => Err(GrantsRevokeError::PartialFailure { output }.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::RefCell,
        sync::{Arc, Mutex},
    };

    use chrono::DateTime;
    use clap::error::ErrorKind;
    use futures::future;
    use rover_client::operations::api_key::pair_list::{OAuthClientPair, PairActor};
    use rover_print::print::testing::TerminalCapture;
    use rstest::rstest;
    use speculoos::prelude::*;
    use tower::service_fn;

    use super::{
        output::tests::{ci_deploy, nightly_checks, revoked, rover},
        *,
    };
    use crate::{RoverErrorCode, options::JsonOutput};

    const TARGET: Target<'static> = Target {
        organization_id: "acme",
        user_id: "user-123",
        rover_client_id: "rover",
    };

    fn pair(client_id: &str, name: &str) -> OAuthClientPair {
        OAuthClientPair {
            client_id: client_id.to_string(),
            name: Some(name.to_string()),
            created_at: DateTime::parse_from_rfc3339("2026-09-25T16:00:00Z").unwrap(),
            created_by: PairActor {
                id: "user-1".to_string(),
                kind: "user".to_string(),
            },
            resources: vec![],
            scopes: vec![],
        }
    }

    fn two_pairs() -> ListOAuthClientsResponse {
        ListOAuthClientsResponse {
            pairs: vec![
                pair("c_8f2a", "ci-deploy"),
                pair("c_91be", "nightly-checks"),
            ],
            next_after: None,
        }
    }

    type PairsResult = Result<ListOAuthClientsResponse, ListOAuthClientsError>;

    /// The client IDs revoked under, in order.
    type Attempted = Arc<Mutex<Vec<String>>>;

    /// Services that answer from fixed results and record every revoke attempted in `attempted`.
    fn services(
        pairs: PairsResult,
        membership: Option<bool>,
        revoke: impl Fn(&str) -> Result<(), RoverClientError> + Send + Sync + 'static,
        attempted: &Attempted,
    ) -> Services<
        impl Service<
            ListOAuthClientsInput,
            Response = ListOAuthClientsResponse,
            Error = ListOAuthClientsError,
        >,
        impl Service<OrgMembershipInput, Response = Option<bool>, Error = RoverClientError>,
        impl Service<RevokeUserGrantsInput, Response = (), Error = RoverClientError>,
    > {
        let pairs = Arc::new(Mutex::new(Some(pairs)));
        let recorded = attempted.clone();
        let revoke = Arc::new(revoke);
        Services {
            pairs: service_fn(move |input: ListOAuthClientsInput| {
                assert_that!(input.organization_id.as_str()).is_equal_to("acme");
                assert_that!(input.limit).is_equal_to(usize::MAX);
                future::ready(pairs.lock().unwrap().take().expect("pairs listed once"))
            }),
            membership: service_fn(move |input: OrgMembershipInput| {
                assert_that!((input.organization_id.as_str(), input.user_id.as_str()))
                    .is_equal_to(("acme", "user-123"));
                future::ready(Ok(membership))
            }),
            revoke: service_fn(move |input: RevokeUserGrantsInput| {
                assert_that!((input.organization_id.as_str(), input.user_id.as_str()))
                    .is_equal_to(("acme", "user-123"));
                recorded.lock().unwrap().push(input.client_id.clone());
                future::ready(revoke(&input.client_id))
            }),
        }
    }

    fn never_asked(_: &str) -> io::Result<bool> {
        panic!("expected no prompt")
    }

    fn all_succeed(_: &str) -> Result<(), RoverClientError> {
        Ok(())
    }

    // FR55/FR62: Rover's client first, then every pair, each reported revoked.
    #[tokio::test]
    async fn a_confirmed_sweep_revokes_under_rover_and_every_pair() {
        let attempted = Attempted::default();
        let services = services(Ok(two_pairs()), Some(true), all_succeed, &attempted);
        let stderr = TerminalCapture::new(false);

        let output = sweep(TARGET, Confirmation::Given, services, &stderr, never_asked)
            .await
            .unwrap();

        assert_that!(output).is_equal_to(RevokeSweepOutput {
            organization_id: "acme".to_string(),
            user_id: "user-123".to_string(),
            user_is_member: Some(true),
            clients: vec![
                revoked(rover()),
                revoked(ci_deploy()),
                revoked(nightly_checks()),
            ],
            cancelled: false,
        });
        assert_that!(*attempted.lock().unwrap()).is_equal_to(vec![
            "rover".to_string(),
            "c_8f2a".to_string(),
            "c_91be".to_string(),
        ]);
        assert_that!(stderr.lines()).is_equal_to(Vec::<String>::new());
    }

    // FR57: the prompt names the user, the organization, and every client, in FR57's text.
    #[tokio::test]
    async fn the_prompt_lists_every_client_it_will_revoke_under() {
        let attempted = Attempted::default();
        let services = services(Ok(two_pairs()), Some(true), all_succeed, &attempted);
        let asked = RefCell::new(None);

        sweep(
            TARGET,
            Confirmation::Ask,
            services,
            &TerminalCapture::new(false),
            |prompt: &str| {
                asked.replace(Some(prompt.to_string()));
                Ok(true)
            },
        )
        .await
        .unwrap();

        assert_that!(asked.into_inner()).is_equal_to(Some(
            "This revokes every grant `user-123` holds under these OAuth clients in organization \
            `acme`:\n  - Rover (personal browser and device-code logins, in every organization)\n  \
            - `ci-deploy` (`c_8f2a`)\n  - `nightly-checks` (`c_91be`)\nIt doesn't revoke access \
            tokens they already hold, and doesn't stop them logging in again.\nRevoke?"
                .to_string(),
        ));
    }

    // FR58: declining revokes nothing and reports `cancelled`.
    #[tokio::test]
    async fn declining_the_prompt_revokes_nothing() {
        let attempted = Attempted::default();
        let services = services(Ok(two_pairs()), Some(true), all_succeed, &attempted);

        let output = sweep(
            TARGET,
            Confirmation::Ask,
            services,
            &TerminalCapture::new(false),
            |_: &str| Ok(false),
        )
        .await
        .unwrap();

        assert_that!(output).is_equal_to(RevokeSweepOutput {
            organization_id: "acme".to_string(),
            user_id: "user-123".to_string(),
            user_is_member: Some(true),
            clients: vec![],
            cancelled: true,
        });
        assert_that!(attempted.lock().unwrap().len()).is_equal_to(0);
    }

    // FR59: no terminal and no `--confirm` fails with E062, before any request.
    #[tokio::test]
    async fn no_terminal_without_confirm_fails_before_any_request() {
        let services = Services {
            pairs: service_fn(|_: ListOAuthClientsInput| -> future::Ready<PairsResult> {
                panic!("expected no request")
            }),
            membership: service_fn(
                |_: OrgMembershipInput| -> future::Ready<Result<Option<bool>, RoverClientError>> {
                    panic!("expected no request")
                },
            ),
            revoke: service_fn(
                |_: RevokeUserGrantsInput| -> future::Ready<Result<(), RoverClientError>> {
                    panic!("expected no request")
                },
            ),
        };

        let err = sweep(
            TARGET,
            Confirmation::Unavailable,
            services,
            &TerminalCapture::new(false),
            never_asked,
        )
        .await
        .unwrap_err();

        assert_that!(err.message()).is_equal_to(
            "Revoking every grant for `user-123` needs confirmation, and there's no terminal to \
            ask on. Pass `--confirm` to proceed without a prompt."
                .to_string(),
        );
        assert_that!(err.code()).is_equal_to(Some(RoverErrorCode::E062));
        insta::assert_json_snapshot!(JsonOutput::from(&err));
    }

    // FR56: if the pairs can't be enumerated, nothing is revoked.
    #[tokio::test]
    async fn a_failed_enumeration_revokes_nothing() {
        let attempted = Attempted::default();
        let services = services(
            Err(ListOAuthClientsError::NoProgress(3)),
            Some(true),
            all_succeed,
            &attempted,
        );

        let err = sweep(
            TARGET,
            Confirmation::Given,
            services,
            &TerminalCapture::new(false),
            never_asked,
        )
        .await
        .unwrap_err();

        assert_that!(err.message()).is_equal_to(format!(
            "Couldn't list organization `acme`'s client-credential pairs, so nothing was revoked: \
            {}",
            ListOAuthClientsError::NoProgress(3)
        ));
        // FR74 names no code for this failure.
        assert_that!(err.code()).is_equal_to(None);
        assert_that!(attempted.lock().unwrap().len()).is_equal_to(0);
    }

    // FR60: a membership lookup that fails means "can't tell" - the sweep still runs, with no
    // warning and `user_is_member: null`.
    #[tokio::test]
    async fn a_failed_membership_lookup_doesnt_stop_the_sweep() {
        let attempted = Attempted::default();
        let Services { pairs, revoke, .. } =
            services(Ok(two_pairs()), Some(true), all_succeed, &attempted);
        let services = Services {
            pairs,
            membership: service_fn(|_: OrgMembershipInput| {
                future::ready(Err(RoverClientError::GrantPermissionDenied {
                    organization_id: "acme".to_string(),
                }))
            }),
            revoke,
        };
        let stderr = TerminalCapture::new(false);

        let output = sweep(TARGET, Confirmation::Given, services, &stderr, never_asked)
            .await
            .unwrap();

        assert_that!(output).is_equal_to(RevokeSweepOutput {
            organization_id: "acme".to_string(),
            user_id: "user-123".to_string(),
            user_is_member: None,
            clients: vec![
                revoked(rover()),
                revoked(ci_deploy()),
                revoked(nightly_checks()),
            ],
            cancelled: false,
        });
        assert_that!(stderr.lines()).is_equal_to(Vec::<String>::new());
    }

    // FR60: a departed user is warned about, and the sweep still runs.
    #[rstest]
    #[case::a_departed_user(Some(false), vec!["Warning: `user-123` isn't a current member of organization `acme`. Rover will still revoke any grants they hold under the clients below.".to_string()])]
    #[case::a_current_member(Some(true), vec![])]
    #[case::membership_unknown(None, vec![])]
    #[tokio::test]
    async fn membership_decides_only_the_warning(
        #[case] membership: Option<bool>,
        #[case] expected_stderr: Vec<String>,
    ) {
        let attempted = Attempted::default();
        let services = services(Ok(two_pairs()), membership, all_succeed, &attempted);
        let stderr = TerminalCapture::new(false);

        let output = sweep(TARGET, Confirmation::Given, services, &stderr, never_asked)
            .await
            .unwrap();

        assert_that!(output.user_is_member).is_equal_to(membership);
        assert_that!(stderr.lines()).is_equal_to(expected_stderr);
        assert_that!(attempted.lock().unwrap().len()).is_equal_to(3);
    }

    // FR61/FR63: a failure under one client doesn't stop the rest, and fails with E063.
    #[tokio::test]
    async fn a_failure_under_one_client_still_attempts_the_rest() {
        let attempted = Attempted::default();
        let services = services(
            Ok(two_pairs()),
            Some(true),
            |client_id| {
                if client_id == "c_8f2a" {
                    Err(RoverClientError::ClientError {
                        msg: "upstream timed out".to_string(),
                    })
                } else {
                    Ok(())
                }
            },
            &attempted,
        );

        let err = sweep(
            TARGET,
            Confirmation::Given,
            services,
            &TerminalCapture::new(false),
            never_asked,
        )
        .await
        .unwrap_err();

        assert_that!(err.code()).is_equal_to(Some(RoverErrorCode::E063));
        assert_that!(err.message())
            .is_equal_to("Revocation failed under 1 of 3 OAuth clients.".to_string());
        // FR67: `data` carries every client's outcome, the failed one with its error.
        insta::assert_json_snapshot!(JsonOutput::from(&err));
        assert_that!(attempted.lock().unwrap().len()).is_equal_to(3);
    }

    // FR73: refused under every client is the permission error, not a partial sweep.
    #[tokio::test]
    async fn refused_under_every_client_is_a_permission_error() {
        let attempted = Attempted::default();
        let services = services(
            Ok(two_pairs()),
            Some(true),
            |_| {
                Err(RoverClientError::GrantPermissionDenied {
                    organization_id: "acme".to_string(),
                })
            },
            &attempted,
        );

        let err = sweep(
            TARGET,
            Confirmation::Given,
            services,
            &TerminalCapture::new(false),
            never_asked,
        )
        .await
        .unwrap_err();

        assert_that!(err.code()).is_equal_to(Some(RoverErrorCode::E053));
        assert_that!(err.message()).is_equal_to(
            "You don't have permission to manage grants across organization `acme`. This \
            requires the organization's grant-management permission."
                .to_string(),
        );
        // No per-client outcomes in `data`: the permission error replaces the sweep's report.
        insta::assert_json_snapshot!(JsonOutput::from(&err));
        assert_that!(attempted.lock().unwrap().len()).is_equal_to(3);
    }

    // FR47/FR48: the sweep form parses; every other combination is a usage error.
    #[rstest]
    #[case::all_without_user(&["--org", "acme", "--all"], ErrorKind::MissingRequiredArgument)]
    #[case::user_without_all(&["--org", "acme", "--user", "user-123"], ErrorKind::MissingRequiredArgument)]
    #[case::user_and_all_without_org(&["--user", "user-123", "--all"], ErrorKind::MissingRequiredArgument)]
    #[case::org_alone(&["--org", "acme"], ErrorKind::MissingRequiredArgument)]
    #[case::a_bare_grant_id(&["g_4c1e", "--org", "acme", "--user", "user-123", "--all"], ErrorKind::UnknownArgument)]
    #[case::nothing_at_all(&[], ErrorKind::MissingRequiredArgument)]
    fn anything_but_the_sweep_form_is_a_usage_error(
        #[case] args: &[&str],
        #[case] expected: ErrorKind,
    ) {
        let err = Revoke::try_parse_from(std::iter::once("revoke").chain(args.iter().copied()))
            .expect_err("expected a usage error");

        assert_that!(err.kind()).is_equal_to(expected);
    }

    #[rstest]
    #[case::without_confirm(&["--org", "acme", "--user", "user-123", "--all"], false)]
    #[case::with_confirm(&["--org", "acme", "--user", "user-123", "--all", "--confirm"], true)]
    fn the_sweep_form_parses(#[case] args: &[&str], #[case] confirm: bool) {
        let revoke = Revoke::try_parse_from(std::iter::once("revoke").chain(args.iter().copied()))
            .expect("expected the sweep form to parse");

        assert_that!((
            revoke.organization_id.as_str(),
            revoke.user_id.as_str(),
            revoke.all,
            revoke.confirm
        ))
        .is_equal_to(("acme", "user-123", true, confirm));
    }
}
