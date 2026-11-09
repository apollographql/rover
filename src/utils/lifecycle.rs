//! The harness every command runs under, so that telemetry sees how it ended: it completed,
//! it panicked, or a termination signal stopped it.

use std::{
    any::Any,
    future::Future,
    panic::{self, AssertUnwindSafe},
    sync::{
        Arc, LazyLock, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use futures::FutureExt;
use sputnik::{Outcome, Session};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

/// Set by the panic hook when a panic happens on any thread.
static PANICKED: AtomicBool = AtomicBool::new(false);

tokio::task_local! {
    /// Set while telemetry reports, so a panic in the reporting task itself is kept off the
    /// terminal. This is a task-local rather than a global flag so a panic in any other task
    /// is still printed and counted.
    static REPORTING: ();
}

/// Wraps the panic hook already installed, so that a panic on any thread is
/// remembered for telemetry, and so that one while telemetry is reporting is
/// kept off the user's terminal. Call it once, after installing the hook that
/// prints panics.
pub fn observe_panics() {
    let print_panic = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        // Only a panic in the reporting task itself is quiet: one on another
        // thread or task is still a panic the user should see and we should count.
        if REPORTING.try_get().is_ok() {
            tracing::debug!(%info, "panicked while reporting telemetry");
        } else {
            PANICKED.store(true, Ordering::SeqCst);
            print_panic(info);
        }
    }));
}

/// Whether a panic has happened on any thread since [`observe_panics`].
pub(crate) fn has_panicked() -> bool {
    PANICKED.load(Ordering::SeqCst)
}

/// Runs a telemetry report, which must change neither the command's output nor its exit
/// code, so a panic inside it is swallowed.
pub(crate) async fn quietly<T>(reporting: impl Future<Output = T>) -> Option<T> {
    REPORTING
        .scope((), AssertUnwindSafe(reporting).catch_unwind())
        .await
        .ok()
}

/// Who reports telemetry for an invocation. The command reports when it ends and a signal
/// watcher reports when a signal arrives, and only one of them can: whichever claims the
/// session first.
pub(crate) struct Telemetry {
    state: Mutex<State>,
}

enum State {
    // Boxed because a session is large next to `Claimed`.
    Unclaimed(Box<Option<Session>>),
    Claimed,
}

/// The right to report, held by whoever claimed it first. `session` is `None` when telemetry
/// is off for this invocation.
pub(crate) struct Claim {
    pub(crate) session: Option<Session>,
}

impl Telemetry {
    pub(crate) fn new(session: Option<Session>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::Unclaimed(Box::new(session))),
        })
    }

    /// `Some` for the first caller only, with the session that caller is to report.
    fn claim(&self) -> Option<Claim> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        match std::mem::replace(&mut *state, State::Claimed) {
            State::Unclaimed(session) => Some(Claim { session: *session }),
            State::Claimed => None,
        }
    }
}

/// How long stopping a command's child processes may delay the exit that follows a signal.
/// It runs alongside the telemetry report, so it shares that report's budget rather than
/// adding to it.
const CHILD_STOP_BUDGET: Duration = Duration::from_millis(250);

/// The child processes a command has started and that must not outlive it. A signal ends
/// Rover by re-raising it, which runs no cleanup, so a child kept running by that is orphaned
/// unless the harness stops it first. A command registers the task that owns each child
/// through [`Children::spawn`], and that task stops the child when [`Children::stopping`] is
/// cancelled. Stopping is abrupt: a child is killed rather than asked to shut down, so one
/// that a terminal has already interrupted doesn't get to finish a graceful shutdown.
pub(crate) struct Children {
    stopping: CancellationToken,
    tasks: TaskTracker,
}

impl Children {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            stopping: CancellationToken::new(),
            tasks: TaskTracker::new(),
        })
    }

    /// The children of this process. Tests use [`Children::new`] instead, because once these
    /// are told to stop, they stay stopped.
    pub(crate) fn global() -> Arc<Self> {
        static GLOBAL: LazyLock<Arc<Children>> = LazyLock::new(Children::new);
        Arc::clone(&GLOBAL)
    }

    /// Cancelled when the harness is about to end the process, and the children must stop.
    pub(crate) fn stopping(&self) -> CancellationToken {
        self.stopping.clone()
    }

    /// Runs the task that owns a child, so that [`Children::stop`] can wait for it to finish.
    pub(crate) fn spawn<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.tasks.spawn(task);
    }

    /// Held while a child is being started, until the task that owns it is registered with
    /// [`Children::spawn`]. [`Children::stop`] waits for it, so a signal that arrives in that
    /// gap doesn't leave the child that is about to appear orphaned.
    pub(crate) fn starting(&self) -> impl Drop + use<> {
        self.tasks.token()
    }

    /// Tells every child to stop and waits for them to, for no longer than `budget`. A child
    /// that is slow to stop is left to the operating system, since it must not hold up the
    /// exit.
    pub(crate) async fn stop(&self, budget: Duration) {
        self.stopping.cancel();
        self.tasks.close();
        if tokio::time::timeout(budget, self.tasks.wait())
            .await
            .is_err()
        {
            tracing::debug!(?budget, "child processes were still stopping at exit");
        }
    }
}

/// A signal that asks the process to stop.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TerminationSignal {
    Interrupt,
    // Only Unix has `SIGTERM`.
    #[cfg_attr(not(unix), allow(dead_code))]
    Terminate,
}

impl TerminationSignal {
    /// Exits the way the process would have if this signal had terminated it by default.
    pub(crate) fn exit_by_default(self) -> ! {
        #[cfg(unix)]
        {
            use signal_hook::consts::{SIGINT, SIGTERM};

            let signal_number = match self {
                TerminationSignal::Interrupt => SIGINT,
                TerminationSignal::Terminate => SIGTERM,
            };
            let _ = signal_hook::low_level::emulate_default_handler(signal_number);
            // The signal should have ended the process already; this is the
            // status a shell reports for one that did.
            std::process::exit(128 + signal_number)
        }
        #[cfg(not(unix))]
        {
            // STATUS_CONTROL_C_EXIT, what Windows reports for a console interrupt.
            std::process::exit(0xC000_013Au32 as i32)
        }
    }
}

/// How a command ended, when it was left to end on its own.
pub(crate) enum Finish<T> {
    /// The command completed, and the right to report is the caller's.
    Completed { output: T, session: Option<Session> },
    /// The command panicked, and the right to report is the caller's.
    Panicked {
        payload: Box<dyn Any + Send>,
        session: Option<Session>,
    },
}

/// Runs a command, stopping it on `SIGTERM` or `SIGINT` even if it handles an interrupt itself
/// or is blocked in synchronous code.
///
/// A signal is noticed by a task of its own, so that a command that never lets the runtime
/// poll it can't hide the signal. That task reports `terminated`, with no duration because it
/// would only say how long the command ran before it was stopped, stops the command's
/// `children`, and then calls `stop`, which is meant to end the process. If the command ends
/// first it gets to report instead, and a signal that arrives while it does is passed straight
/// to `stop`.
pub(crate) async fn run<F: Future>(
    telemetry: &Arc<Telemetry>,
    children: &Arc<Children>,
    stop: impl FnOnce(TerminationSignal) + Send + 'static,
    command: F,
) -> Finish<F::Output> {
    // Listening before the command starts means a signal in its first moments isn't missed.
    let signals = Signals::listen();
    let watcher = Arc::clone(telemetry);
    let children = Arc::clone(children);
    tokio::spawn(async move {
        let signal = signals.received().await;
        let report = async {
            if let Some(Claim {
                session: Some(session),
            }) = watcher.claim()
            {
                quietly(session.with_outcome(Outcome::Terminated).report()).await;
            }
        };
        tokio::join!(report, children.stop(CHILD_STOP_BUDGET));
        stop(signal);
    });

    let result = AssertUnwindSafe(command).catch_unwind().await;
    let Some(Claim { session }) = telemetry.claim() else {
        // A signal got there first, and is ending the process.
        return std::future::pending().await;
    };
    match result {
        Ok(output) => Finish::Completed { output, session },
        Err(payload) => Finish::Panicked { payload, session },
    }
}

#[cfg(unix)]
struct Signals {
    interrupt: Option<tokio::signal::unix::Signal>,
    terminate: Option<tokio::signal::unix::Signal>,
}

#[cfg(unix)]
impl Signals {
    fn listen() -> Self {
        use tokio::signal::unix::{SignalKind, signal};

        Self {
            interrupt: signal(SignalKind::interrupt()).ok(),
            terminate: signal(SignalKind::terminate()).ok(),
        }
    }

    async fn received(mut self) -> TerminationSignal {
        async fn next(signal: &mut Option<tokio::signal::unix::Signal>) {
            match signal {
                Some(signal) => {
                    signal.recv().await;
                }
                None => std::future::pending().await,
            }
        }
        tokio::select! {
            () = next(&mut self.interrupt) => TerminationSignal::Interrupt,
            () = next(&mut self.terminate) => TerminationSignal::Terminate,
        }
    }
}

#[cfg(windows)]
struct Signals {
    ctrl_c: Option<tokio::signal::windows::CtrlC>,
}

#[cfg(windows)]
impl Signals {
    fn listen() -> Self {
        Self {
            ctrl_c: tokio::signal::windows::ctrl_c().ok(),
        }
    }

    async fn received(mut self) -> TerminationSignal {
        match self.ctrl_c.as_mut() {
            Some(ctrl_c) => {
                ctrl_c.recv().await;
                TerminationSignal::Interrupt
            }
            None => std::future::pending().await,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };

    use serial_test::serial;
    use signal_hook::{
        consts::{SIGINT, SIGTERM},
        low_level::raise,
    };
    use speculoos::prelude::*;
    use tokio::sync::oneshot;

    use super::{Children, Finish, Telemetry, TerminationSignal, quietly, run};

    /// What a [`Finish`] came to, in a form that can be compared whole.
    #[derive(Debug, PartialEq, Eq)]
    enum Ended {
        Completed(i32),
        Panicked,
    }

    impl From<Finish<i32>> for Ended {
        fn from(finish: Finish<i32>) -> Self {
            match finish {
                Finish::Completed { output, .. } => Ended::Completed(output),
                Finish::Panicked { .. } => Ended::Panicked,
            }
        }
    }

    /// A stop that reports the signal it was given, in place of ending the test process.
    fn stop_with(
        sender: oneshot::Sender<TerminationSignal>,
    ) -> impl FnOnce(TerminationSignal) + Send + 'static {
        move |signal| {
            let _ = sender.send(signal);
        }
    }

    #[rstest::rstest]
    #[case::a_value(false, Some(7))]
    #[case::a_panic(true, None)]
    #[tokio::test]
    async fn quietly_returns_the_value_and_swallows_a_panic(
        #[case] panics: bool,
        #[case] expected: Option<i32>,
    ) {
        let reported = quietly(async move {
            if panics {
                panic!("boom");
            }
            7
        })
        .await;

        assert_that!(reported).is_equal_to(expected);
    }

    #[test]
    fn only_the_first_to_claim_the_session_reports() {
        let telemetry = Telemetry::new(None);
        assert_that!(telemetry.claim().map(|claim| claim.session.is_none()))
            .is_equal_to(Some(true));
        assert_that!(telemetry.claim().map(|claim| claim.session.is_none())).is_equal_to(None);
    }

    #[tokio::test]
    #[serial(signals)]
    async fn a_completed_command_finishes_with_its_output() {
        let (stop, _) = oneshot::channel();
        let finish = run(
            &Telemetry::new(None),
            &Children::new(),
            stop_with(stop),
            async { 7 },
        )
        .await;
        assert_that!(Ended::from(finish)).is_equal_to(Ended::Completed(7));
    }

    #[tokio::test]
    #[serial(signals)]
    async fn a_panicking_command_is_caught() {
        let (stop, _) = oneshot::channel();
        let finish = run(
            &Telemetry::new(None),
            &Children::new(),
            stop_with(stop),
            async { panic!("boom") },
        )
        .await;
        assert_that!(Ended::from(finish)).is_equal_to(Ended::Panicked);
    }

    /// Runs `command` until a signal stops it, and says which signal that was.
    async fn signal_received_by(
        signal: i32,
        command: impl Future<Output = i32> + Send + 'static,
    ) -> Result<TerminationSignal, String> {
        let (sender, receiver) = oneshot::channel();
        let (started, running) = oneshot::channel();
        tokio::spawn(async move {
            let command = async move {
                let _ = started.send(());
                command.await
            };
            run(
                &Telemetry::new(None),
                &Children::new(),
                stop_with(sender),
                command,
            )
            .await;
        });
        // `run` listens for signals before it polls the command, so once the command
        // has started, a signal is certain to be noticed.
        running.await.unwrap();
        raise(signal).unwrap();
        match tokio::time::timeout(Duration::from_secs(2), receiver).await {
            Ok(Ok(received)) => Ok(received),
            Ok(Err(_)) => Err("the harness dropped its stop".to_string()),
            Err(_) => Err("the signal was never noticed".to_string()),
        }
    }

    #[rstest::rstest]
    #[case::sigterm(SIGTERM, TerminationSignal::Terminate)]
    #[case::sigint(SIGINT, TerminationSignal::Interrupt)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    #[serial(signals)]
    async fn a_signal_stops_a_running_command(
        #[case] signal: i32,
        #[case] expected: TerminationSignal,
    ) {
        let received = signal_received_by(signal, async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            0
        })
        .await;
        assert_that!(received).is_equal_to(Ok(expected));
    }

    /// A command that handles an interrupt itself, like `rover dev`, is stopped too.
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    #[serial(signals)]
    async fn a_command_that_handles_an_interrupt_itself_is_stopped() {
        let received = signal_received_by(SIGINT, async {
            let _ = tokio::signal::ctrl_c().await;
            // A command's own handler would carry on running cleanup here.
            tokio::time::sleep(Duration::from_secs(60)).await;
            0
        })
        .await;
        assert_that!(received).is_equal_to(Ok(TerminationSignal::Interrupt));
    }

    /// Runs a command that has started a child, which `child` is given the harness's
    /// `Children` to own, then sends `SIGTERM` and says what the child had done by the time
    /// the harness handed over to `stop`.
    async fn child_state_at_stop<Fut: Future<Output = ()> + Send + 'static>(
        child: impl FnOnce(Arc<Children>, Arc<AtomicBool>) -> Fut,
    ) -> Result<(bool, Duration), String> {
        let children = Children::new();
        let stopped = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = oneshot::channel();
        let (started, running) = oneshot::channel();
        let child = child(Arc::clone(&children), Arc::clone(&stopped));
        children.spawn(child);
        let observed = Arc::clone(&stopped);
        tokio::spawn(async move {
            let command = async move {
                let _ = started.send(());
                std::future::pending::<i32>().await
            };
            let stop = move |_| {
                let _ = sender.send((observed.load(Ordering::SeqCst), Instant::now()));
            };
            run(&Telemetry::new(None), &children, stop, command).await;
        });
        running.await.unwrap();
        let raised = Instant::now();
        raise(SIGTERM).unwrap();
        match tokio::time::timeout(Duration::from_secs(2), receiver).await {
            Ok(Ok((was_stopped, at))) => Ok((was_stopped, at.duration_since(raised))),
            Ok(Err(_)) => Err("the harness dropped its stop".to_string()),
            Err(_) => Err("the harness never handed over to stop".to_string()),
        }
    }

    /// A child's owner is told to stop it, and has finished doing so before the process is
    /// ended, which is what keeps the child from being orphaned.
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    #[serial(signals)]
    async fn children_are_stopped_before_the_process_is_ended() {
        let was_stopped = child_state_at_stop(|children, stopped| async move {
            children.stopping().cancelled().await;
            // Stopping a real child takes a moment, which the harness must wait out.
            tokio::time::sleep(Duration::from_millis(50)).await;
            stopped.store(true, Ordering::SeqCst);
        })
        .await;
        assert_that!(was_stopped.map(|(stopped, _)| stopped)).is_equal_to(Ok(true));
    }

    /// A child that won't stop can't keep the process from ending.
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    #[serial(signals)]
    async fn a_child_that_will_not_stop_does_not_hold_up_the_exit() {
        let (was_stopped, waited) = child_state_at_stop(|_, _| std::future::pending())
            .await
            .unwrap();
        assert_that!(was_stopped).is_equal_to(false);
        // The budget is 250ms; the rest is slack for a loaded machine.
        assert_that!(waited).is_less_than(Duration::from_millis(750));
    }

    /// A command blocked in synchronous code, like a prompt or a read from stdin, never lets
    /// the runtime poll it, but a signal still stops it. As in `main`, the command is driven
    /// by `block_on` on a thread that isn't one of the runtime's workers.
    #[rstest::rstest]
    #[case::sigterm(SIGTERM, TerminationSignal::Terminate)]
    #[case::sigint(SIGINT, TerminationSignal::Interrupt)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    #[serial(signals)]
    async fn a_signal_stops_a_command_blocked_in_synchronous_code(
        #[case] signal: i32,
        #[case] expected: TerminationSignal,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        let (started, running) = std::sync::mpsc::channel();
        let stop = move |signal| {
            let _ = sender.send(signal);
        };
        // The command blocks this thread, so the signal is sent and watched for from another,
        // once the command has started and so the handlers are certain to be registered.
        let observer = std::thread::spawn(move || {
            running.recv().unwrap();
            raise(signal).unwrap();
            receiver.recv_timeout(Duration::from_secs(2))
        });
        // The command finishes after its sleep, once the signal has been dealt with, and then
        // waits forever as it would in a process that was exiting.
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            run(&Telemetry::new(None), &Children::new(), stop, async move {
                started.send(()).unwrap();
                std::thread::sleep(Duration::from_secs(2));
                0
            }),
        )
        .await;
        assert_that!(observer.join().unwrap()).is_equal_to(Ok(expected));
    }
}
