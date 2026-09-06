//! Syntax model for STF commands.
//!
//! Leaf strings retain their source spelling, compound actions and matches
//! preserve input order, and statement variants follow command order. For
//! example, `packet 1 00ff` retains `"1"` as its port and `"00ff"` as its
//! packet data.

use crate::lang::common::source::Phrase;

pub use super::name::Name;

// == Leaf syntax

pub type Id = String;
pub type Number = String;
pub type Port = String;
pub type Handle = String;
pub type Packet = String;
pub type ExpectedPacket = String;
pub type Session = String;
pub type Program = Vec<Phrase<Statement>>;

// == Compound syntax

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Argument {
    pub id: Id,
    pub number: Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub name: Name,
    pub args: Vec<Argument>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchKind {
    Number(Number),
    Slash(Number, Number),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableMatch {
    pub name: Name,
    pub kind: MatchKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CounterTarget {
    Id(Id),
    Index(Number),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Eq,
    Ne,
    Le,
    Lt,
    Ge,
    Gt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterKind {
    Bytes,
    Packets,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterCheck {
    pub kind: Option<CounterKind>,
    pub condition: Condition,
    pub number: Number,
}

// == Statements

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Statement {
    Wait,
    RemoveAll,
    Expect {
        port: Port,
        packet_expected: Option<ExpectedPacket>,
        exact: bool,
    },
    Packet {
        port: Port,
        packet: Packet,
    },
    NoPacket,
    Add {
        table: Name,
        priority: Option<i64>,
        matches: Vec<TableMatch>,
        action: Action,
        id: Option<Id>,
    },
    SetDefault {
        table: Name,
        action: Action,
    },
    CheckCounter {
        counter: Id,
        target: CounterTarget,
        check: CounterCheck,
    },
    MirroringAdd {
        session: Session,
        port: Port,
    },
    MirroringAddMc {
        session: Session,
        group_id: Id,
    },
    MirroringGet {
        session: Session,
    },
    McGroupCreate {
        group_id: Id,
    },
    McNodeCreate {
        replication_id: Id,
        ports: Vec<Port>,
    },
    McNodeAssociate {
        group_id: Id,
        handle: Handle,
    },
    RegisterRead {
        name: Name,
        index: Number,
    },
    RegisterWrite {
        name: Name,
        index: Number,
        value: Number,
    },
    RegisterReset {
        name: Name,
    },
}
