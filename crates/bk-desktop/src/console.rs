//! The debugger on the terminal: commands are read from stdin on a thread,
//! so the window stays responsive while the machine is stopped.

use std::io::{BufRead, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use bk::debugger::Action;
use bk::{Break, Debugger, Machine};

/// Frames a headless `c` may run before giving up (one minute).
const HEADLESS_RUN_FRAMES: u32 = 3000;

pub struct Console {
    debugger: Debugger,
    input: Option<Receiver<String>>,
    /// The machine is stopped in the debugger.
    pub stopped: bool,
}

fn prompt() {
    print!("bk> ");
    let _ = std::io::stdout().flush();
}

impl Console {
    pub fn new() -> Self {
        Console {
            debugger: Debugger::new(),
            input: None,
            stopped: false,
        }
    }

    /// Stop the machine and show where it is.
    pub fn enter(&mut self, m: &Machine) {
        if self.stopped {
            return;
        }
        if self.input.is_none() {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
            self.input = Some(rx);
            println!("debugger: h for help, c to continue");
        }
        self.stopped = true;
        print!("{}", self.debugger.status(m));
        prompt();
    }

    /// While stopped: run any commands that have arrived.
    pub fn poll(&mut self, m: &mut Machine) {
        let Some(input) = &self.input else { return };
        loop {
            match input.try_recv() {
                Ok(line) => {
                    let (out, action) = self.debugger.command(m, &line);
                    print!("{out}");
                    if action == Action::Run {
                        self.stopped = false;
                        let _ = std::io::stdout().flush();
                        return;
                    }
                    prompt();
                }
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    println!("\nend of debugger input: breakpoints cleared, running");
                    self.debugger.detach(m);
                    self.stopped = false;
                    self.input = None;
                    return;
                }
            }
        }
    }

    /// The running machine hit a breakpoint or watchpoint.
    /// The running machine hit a breakpoint or watchpoint. Returns false
    /// if it was a breakpoint whose condition doesn't hold (the machine has
    /// been stepped past it and should keep running).
    pub fn on_break(&mut self, m: &mut Machine, why: Break) -> bool {
        let Some(text) = self.debugger.stopped(m, why) else {
            return false;
        };
        println!();
        print!("{text}");
        self.stopped = true;
        prompt();
        true
    }

    /// Add symbols (see [`bk::Debugger::load_symbols`]).
    pub fn load_symbols(&mut self, text: &str) -> usize {
        self.debugger.load_symbols(text)
    }

    /// While tracing, write out what ran.
    pub fn flush_trace(&mut self, m: &mut Machine) {
        if self.debugger.tracing() {
            self.debugger.flush_trace(m);
        }
    }

    /// Headless: read commands from stdin until it ends. `c` and `n` run
    /// the machine until they stop (for up to a minute of emulated time).
    pub fn run_headless(&mut self, m: &mut Machine) {
        print!("{}", self.debugger.status(m));
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            println!("bk> {line}");
            let (out, action) = self.debugger.command(m, &line);
            print!("{out}");
            if action == Action::Run {
                match self.debugger.run(m, HEADLESS_RUN_FRAMES) {
                    Some(out) => print!("{out}"),
                    None => println!("still running after {HEADLESS_RUN_FRAMES} frames"),
                }
            }
        }
    }
}
