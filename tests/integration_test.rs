extern crate py_spy;

use py_spy::{Config, PythonSpy};

struct TestRunner {
    child: std::process::Child,
    spy: PythonSpy
}

impl TestRunner {
    fn new(config: Config, filename: &str) -> TestRunner {
        let child = std::process::Command::new("python").arg(filename).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let spy = PythonSpy::retry_new(child.id() as _, &config, 20).unwrap();

        TestRunner{child, spy}
    }
}

impl Drop for TestRunner {
    fn drop(&mut self) {
        if let Err(err) = self.child.kill() {
            eprintln!("Failed to kill child process {}", err);
        }
    }
}

#[test]
fn test_busy_loop() {
    #[cfg(target_os="macos")]
    {
        // We need root permissions here to run this on OSX
        if unsafe { libc::geteuid() } != 0 {
            return;
        }
    }
    let mut runner = TestRunner::new(Config::default(), "./tests/scripts/busyloop.py");
    let traces = runner.spy.get_stack_traces().unwrap();

    // we can't be guaranteed what line the script is processing, but
    // we should be able to say that the script is active and
    // catch issues like https://github.com/benfred/py-spy/issues/141
    assert!(traces[0].active);
}

#[cfg(unwind)]
#[test]
fn test_thread_reuse() {
    // on linux we had an issue with the pthread -> native thread id caching
    // the problem was that the pthreadids were getting re-used,
    // and this caused errors on native unwind (since the native thread had
    // exitted). Test that this works with a simple script that creates
    // a couple short lived threads, and then profiling with native enabled
    let config = Config{native: true, ..Default::default()};
    let mut runner = TestRunner::new(config, "./tests/scripts/thread_reuse.py");

    let mut errors = 0;

    for _ in 0..100 {
        // should be able to get traces here BUT we do sometimes get errors about
        // not being able to suspend process ("No such file or directory (os error 2)"
        // when threads exit. Allow a small number of errors here.
        if let Err(e) = runner.spy.get_stack_traces() {
            println!("Failed to get traces {}", e);
            errors += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    assert!(errors <= 3);
}

#[test]
fn test_long_sleep() {
    #[cfg(target_os="macos")]
    {
        // We need root permissions here to run this on OSX
        if unsafe { libc::geteuid() } != 0 {
            return;
        }
    }

    let mut runner = TestRunner::new(Config::default(), "./tests/scripts/longsleep.py");

    let traces = runner.spy.get_stack_traces().unwrap();
    assert_eq!(traces.len(), 1);
    let trace = &traces[0];

    // Make sure the stack trace is what we expect
    assert_eq!(trace.frames[0].name, "longsleep");
    assert_eq!(trace.frames[0].filename, "./tests/scripts/longsleep.py");
    assert_eq!(trace.frames[0].line, 5);

    assert_eq!(trace.frames[1].name, "<module>");
    assert_eq!(trace.frames[1].line, 9);
    assert_eq!(trace.frames[0].filename, "./tests/scripts/longsleep.py");

    assert!(!traces[0].owns_gil);

    // we should reliably be able to detect the thread is sleeping on osx/windows
    // linux+freebsd is trickier
    #[cfg(any(target_os="macos", target_os="windows"))]
    assert!(!traces[0].active);
}

#[test]
fn test_recursive() {
    #[cfg(target_os="macos")]
    {
        // We need root permissions here to run this on OSX
        if unsafe { libc::geteuid() } != 0 {
            return;
        }
    }

    // there used to be a problem where the top-level functions being returned
    // weren't actually entry points: https://github.com/benfred/py-spy/issues/56
    // This was fixed by locking the process while we are profling it. Test that
    // the fix works by generating some samples from a program that would exhibit
    // this behaviour
    let mut runner = TestRunner::new(Config::default(), "./tests/scripts/recursive.py");

    for _ in 0..100 {
        let traces = runner.spy.get_stack_traces().unwrap();
        assert_eq!(traces.len(), 1);
        let trace = &traces[0];

        assert!(trace.frames.len() <= 22);

        let top_level_frame = &trace.frames[trace.frames.len()-1];
        assert_eq!(top_level_frame.name, "<module>");
        assert!((top_level_frame.line == 8) || (top_level_frame.line == 7));

        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn test_unicode() {
    #[cfg(target_os="macos")]
    {
        if unsafe { libc::geteuid() } != 0 {
            return;
        }
    }
    let mut runner = TestRunner::new(Config::default(), "./tests/scripts/unicode💩.py");

    let traces = runner.spy.get_stack_traces().unwrap();
    assert_eq!(traces.len(), 1);
    let trace = &traces[0];

    assert_eq!(trace.frames[0].name, "function1");
    assert_eq!(trace.frames[0].filename, "./tests/scripts/unicode💩.py");
    assert_eq!(trace.frames[0].line, 6);

    assert_eq!(trace.frames[1].name, "<module>");
    assert_eq!(trace.frames[1].line, 9);
    assert_eq!(trace.frames[0].filename, "./tests/scripts/unicode💩.py");

    assert!(!traces[0].owns_gil);
}

#[test]
fn test_local_vars() {
    #[cfg(target_os="macos")]
    {
        // We need root permissions here to run this on OSX
        if unsafe { libc::geteuid() } != 0 {
            return;
        }
    }

    let config = Config{dump_locals: true, ..Default::default()};
    let mut runner = TestRunner::new(config, "./tests/scripts/local_vars.py");

    let traces = runner.spy.get_stack_traces().unwrap();
    assert_eq!(traces.len(), 1);
    let trace = &traces[0];
    assert_eq!(trace.frames.len(), 2);
    let frame = &trace.frames[0];
    let locals = frame.locals.as_ref().unwrap();

    assert_eq!(locals.len(), 8);

    let arg1 = &locals[0];
    assert_eq!(arg1.name, "arg1");
    assert!(arg1.arg);
    assert_eq!(arg1.repr, Some("\"foo\"".to_owned()));

    let arg2 = &locals[1];
    assert_eq!(arg2.name, "arg2");
    assert!(arg2.arg);
    assert_eq!(arg2.repr, Some("None".to_owned()));

    let arg3 = &locals[2];
    assert_eq!(arg3.name, "arg3");
    assert!(arg3.arg);
    assert_eq!(arg3.repr, Some("True".to_owned()));

    let local1 = &locals[3];
    assert_eq!(local1.name, "local1");
    assert!(!local1.arg);
    assert_eq!(local1.repr, Some("[-1234, 5678]".to_owned()));

    let local2 = &locals[4];
    assert_eq!(local2.name, "local2");
    assert!(!local2.arg);
    assert_eq!(local2.repr, Some("(\"a\", \"b\", \"c\")".to_owned()));

    let local3 = &locals[5];
    assert_eq!(local3.name, "local3");
    assert!(!local3.arg);
    assert_eq!(local3.repr, Some("123456789123456789".to_owned()));

    let local4 = &locals[6];
    assert_eq!(local4.name, "local4");
    assert!(!local4.arg);
    assert_eq!(local4.repr, Some("3.1415".to_owned()));

    let local5 = &locals[7];
    assert_eq!(local5.name, "local5");
    assert!(!local5.arg);

    // we only support dictionary lookup on python 3.6+ right now
    if runner.spy.version.major == 3 && runner.spy.version.minor >= 6 {
        assert_eq!(local5.repr, Some("{\"a\": False, \"b\": (1, 2, 3)}".to_owned()));
    }
}