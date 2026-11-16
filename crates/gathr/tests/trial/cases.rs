//! The trial's cases, who runs them and in what order (0066, R4 to R6). A case is one fixed test of
//! what an agent does; a block is one arm and one run of every case still due.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    T1,
    T2,
    T5,
    T8,
}

impl Case {
    pub fn name(self) -> &'static str {
        match self {
            Case::T1 => "T1",
            Case::T2 => "T2",
            Case::T5 => "T5",
            Case::T8 => "T8",
        }
    }

    /// The line it measures, by its id in `gathr::roles::SHIPPED`.
    pub fn line(self) -> &'static str {
        match self {
            Case::T5 => "L1",
            Case::T1 => "L2",
            Case::T8 => "L3",
            Case::T2 => "L4",
        }
    }

    /// Its limits: input tokens as the engines report them, cache included, and seconds.
    pub fn limits(self) -> (u64, u64) {
        match self {
            Case::T1 => (4_000_000, 30 * 60),
            Case::T2 | Case::T5 => (1_500_000, 10 * 60),
            Case::T8 => (1_000_000, 5 * 60),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Claude,
    Codex,
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::Claude => "claude",
            Engine::Codex => "codex",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm {
    With,
    Without,
}

impl Arm {
    pub fn name(self) -> &'static str {
        match self {
            Arm::With => "with",
            Arm::Without => "without",
        }
    }
}

/// An agent of the trial and the case it runs, on the room's model and effort for that role and
/// engine (the detail's section 9, fact 14).
#[derive(Debug, PartialEq)]
pub struct Seat {
    pub case: Case,
    pub engine: Engine,
    pub agent: &'static str,
    pub model: &'static str,
    pub effort: &'static str,
}

/// The paused lead who gives the tasks and sends T8's message, and T5's member: no turn of hers runs.
pub const GIVER: &str = "Ana";

pub const SEATS: [Seat; 6] = [
    Seat {
        case: Case::T1,
        engine: Engine::Claude,
        agent: "Cy",
        model: "opus",
        effort: "high",
    },
    Seat {
        case: Case::T8,
        engine: Engine::Claude,
        agent: "Dot",
        model: "opus",
        effort: "high",
    },
    Seat {
        case: Case::T5,
        engine: Engine::Claude,
        agent: "Bo",
        model: "opus",
        effort: "high",
    },
    Seat {
        case: Case::T1,
        engine: Engine::Codex,
        agent: "Eli",
        model: "gpt-6.1-sol",
        effort: "medium",
    },
    Seat {
        case: Case::T8,
        engine: Engine::Codex,
        agent: "Fin",
        model: "gpt-6.1-sol",
        effort: "medium",
    },
    Seat {
        case: Case::T2,
        engine: Engine::Codex,
        agent: "Gus",
        model: "latest:sol",
        effort: "high",
    },
];

/// One arm and one run of each case due, each in its own chat, all at once.
#[derive(Debug)]
pub struct Block {
    pub n: u64,
    pub arm: Arm,
    pub seats: Vec<&'static Seat>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Every round, both arms.
    Full,
    /// The first small run: one T1 and one T8 on Claude Code, with the lines only (section 7, step 4).
    First,
    /// The first round on the replay stand-in engines, to prove the harness.
    StandIn,
}

/// No block starts once the trial has spent this many input tokens.
pub const CAP: u64 = 90_000_000;

/// The blocks, in order.
/// Eight rounds of both arms, the arm that goes first alternating by round, so both see the same
/// hours and engine copies: T5 in every round, the other cases in the first six (R6's runs).
pub fn plan(mode: Mode) -> Vec<Block> {
    if mode == Mode::First {
        let first = |s: &&Seat| s.engine == Engine::Claude && matches!(s.case, Case::T1 | Case::T8);
        let seats = SEATS.iter().filter(first).collect();
        return vec![Block {
            n: 1,
            arm: Arm::With,
            seats,
        }];
    }
    let rounds = if mode == Mode::StandIn { 1 } else { 8 };
    let mut blocks = Vec::new();
    for round in 1..=rounds {
        let arms = match round % 2 {
            1 => [Arm::With, Arm::Without],
            _ => [Arm::Without, Arm::With],
        };
        for arm in arms {
            let due = |s: &&Seat| s.case == Case::T5 || round <= 6;
            let seats = SEATS.iter().filter(due).collect();
            blocks.push(Block {
                n: blocks.len() as u64 + 1,
                arm,
                seats,
            });
        }
    }
    blocks
}

/// Whether another block may start once `spent` input tokens are spent.
pub fn may_start(spent: u64) -> bool {
    spent < CAP
}

/// The most the plan can spend before its turns run past a limit: every run to its token limit, or
/// the cap plus the largest block's limits when that is less.
pub fn most(blocks: &[Block]) -> u64 {
    let limits = |b: &Block| b.seats.iter().map(|s| s.case.limits().0).sum::<u64>();
    let all: u64 = blocks.iter().map(limits).sum();
    let largest = blocks.iter().map(limits).max().unwrap_or(0);
    all.min(CAP + largest)
}
